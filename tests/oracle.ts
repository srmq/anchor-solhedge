import * as anchor from "@coral-xyz/anchor";
import { AnchorSolhedge } from "../target/types/anchor_solhedge";
import { getUserSettleTicketAccountAddressForVaultFactory, getUserTicketAccountAddressForVaultFactory } from "./accounts";
import axios from 'axios'
import { cdfStdNormal, convertInterest, volatilitySquared } from "./stats";
import * as token from "@solana/spl-token"
import * as dotenv from "dotenv";
import { CandleGranularity, TokenPrice, granularityToSeconds } from "./util";
import { snakeDollarMintAddr, snakeBTCMintAddr } from "./snake-minter-devnet";

dotenv.config()

const ORACLE_KEY = JSON.parse(process.env.DEVNET_ORACLE_KEY) as number[];
const HELLO_MOON_BEARER = process.env.HELLO_MOON_BEARER;
const ANCHOR_FREEZE_SECONDS = 30 * 60;
const STEP_SAMPLE_SIZE = 30;

// SHOULD BE false on mainnet!
const DEVNET_MODE = true;

var devnetMockMintTranslator = undefined
if (DEVNET_MODE) {
    devnetMockMintTranslator = []
    devnetMockMintTranslator[snakeBTCMintAddr.toBase58()] = '3NZ9JMVBmGAqocybic2c7LQCJScmgsAZ6vQqTDzcqmJh'
    devnetMockMintTranslator[snakeDollarMintAddr.toBase58()] = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v'
}

//should decrease this (e.g. to 5*60) when hello moon give more reliable data
const CURRENT_PRICE_MAX_DELAY_SECONDS = 20*60
const RISK_FREE_YEARLY_RATE = 0.06;
//should decrease this (e.g. to 3) when hello moon give more reliable data
const MAX_STEPS_TO_TOO_OLD = 6;

export const oracleAddr = new anchor.web3.PublicKey(process.env.DEVNET_ORACLE_PUBKEY)

const axiosDefaultOptions = {
    baseURL: 'https://rest-api.hellomoon.io',
    headers: {
        'Accept': 'application/json',
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${HELLO_MOON_BEARER}`
    }
}

const axiosInstance = axios.create(axiosDefaultOptions)

class SupportedAssets {
    private readonly assets: Set<string>
    constructor() {
        this.assets = new Set<string>([
            '3NZ9JMVBmGAqocybic2c7LQCJScmgsAZ6vQqTDzcqmJh,EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v' // (Wormhole wBTC, USDC)
        ])
        if (DEVNET_MODE) {
            const mockPair = snakeBTCMintAddr.toBase58() + ',' + snakeDollarMintAddr.toBase58()
            this.assets.add(mockPair)
        }
    }
    public isSupported(baseAsset: anchor.web3.PublicKey, quoteAsset: anchor.web3.PublicKey): boolean {
        return this.assets.has(baseAsset.toBase58() + ',' + quoteAsset.toBase58())
    }

}

const supportedAssets = new SupportedAssets()

export const getOraclePubKey = (): anchor.web3.PublicKey => {
    const secretKey = Uint8Array.from(ORACLE_KEY)
    const keypair = anchor.web3.Keypair.fromSecretKey(secretKey)
    return keypair.publicKey
}

export const updatePutOptionSettlePrice = async (
    program: anchor.Program<AnchorSolhedge>,
    vaultFactoryInfo: anchor.web3.PublicKey,
    user: anchor.web3.PublicKey
): Promise<string> => {
    const settleTicketAddress = await getUserSettleTicketAccountAddressForVaultFactory(program, vaultFactoryInfo, user)
    const ticketAccount = await program.account.putOptionSettlePriceTicketInfo.fetch(settleTicketAddress)
    if (ticketAccount == undefined || ticketAccount.isUsed) {
        throw new Error("Unexistent or used ticket")
    }
    const vaultFactoryAccount = await program.account.putOptionVaultFactoryInfo.fetch(vaultFactoryInfo)
    if (!supportedAssets.isSupported(vaultFactoryAccount.baseAsset, vaultFactoryAccount.quoteAsset)) {
        throw new Error('The pair of (base asset, quote asset) in this vault factory is not supported by the Oracle')
    }
    const epochInSeconds = Math.floor(Date.now() / 1000);
    const maturity = vaultFactoryAccount.maturity.toNumber()
    if (maturity >= epochInSeconds) {
        throw new Error('This put option has not yet reached maturity')
    }
    if (epochInSeconds - maturity < 60) {
        throw new Error('Please wait at least 1 minute after maturity to settle option')
    }
    let maturityMinute = maturity - (maturity % 60)
    console.log(`Will try to get one minute candles from ${maturityMinute - 60*60} and ${maturityMinute+1}`)
    let candles = await getCandlesticksBetween(vaultFactoryAccount.baseAsset.toString(), maturityMinute - 60*60, maturityMinute+1, CandleGranularity.FIVE_MIN)

    if (candles.length == 0) {
        throw new Error(`Could not get candle stick price data around maturity`)
    }

    //sorting by decreasing startTime
    candles.sort((a, b) => (a.startTime > b.startTime ? -1 : 1))

    //checking if candles are unexpectedly too old
    if (maturityMinute - candles[0].startTime > MAX_STEPS_TO_TOO_OLD*granularityToSeconds(CandleGranularity.FIVE_MIN)) {
        throw Error(`Cannot trust datafeed, candle stick data is sparse on maturity. Last startTime epoch was: ${candles[0].startTime}`)
    }

    let settlePrice = candles[0]["close"]
    if (settlePrice == undefined || settlePrice <= 0) {
        throw new Error(`Invalid settle price: ${settlePrice}`)
    }
    const d = new Date(0)
    d.setUTCSeconds(maturity)
    const conn = program.provider.connection
    const mintQuoteAsset = await token.getMint(conn, vaultFactoryAccount.quoteAsset)
    console.log(`The price at maturity (${d.toUTCString()}) was ${settlePrice/(10**(mintQuoteAsset.decimals))} dollars`)
    const oracleKeyPair = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(ORACLE_KEY))
    let tx = program.methods.oracleUpdateSettlePrice(new anchor.BN(settlePrice)).accounts({
        vaultFactoryInfo: vaultFactoryInfo,
        updateTicket: settleTicketAddress,
        ticketOwner: user,
        initializer: oracleKeyPair.publicKey
    }).signers([oracleKeyPair]).rpc()
    return tx
}

export const updatePutOptionFairPrice = async (
    program: anchor.Program<AnchorSolhedge>,
    vaultFactoryInfo: anchor.web3.PublicKey,
    user: anchor.web3.PublicKey
): Promise<string> => {
    const ticketAddress = await getUserTicketAccountAddressForVaultFactory(program, vaultFactoryInfo, user)
    const ticketAccount = await program.account.putOptionUpdateFairPriceTicketInfo.fetch(ticketAddress)
    if (ticketAccount == undefined || ticketAccount.isUsed) {
        throw new Error("Unexistent or used ticket")
    }
    const vaultFactoryAccount = await program.account.putOptionVaultFactoryInfo.fetch(vaultFactoryInfo)
    if (!supportedAssets.isSupported(vaultFactoryAccount.baseAsset, vaultFactoryAccount.quoteAsset)) {
        throw new Error('The pair of (base asset, quote asset) in this vault factory is not supported by the Oracle')
    }
    const epochInSeconds = Math.floor(Date.now() / 1000);
    const maturity = vaultFactoryAccount.maturity.toNumber()
    if (vaultFactoryAccount.matured || maturity < epochInSeconds) {
        throw new Error('This put option has already reached maturity')
    }
    if (maturity < epochInSeconds + ANCHOR_FREEZE_SECONDS) {
        throw new Error('This put option is already frozen')
    }

    //Ok, now we now maturity is in the future and the option is not already frozen (30 minutes to maturity)
    //If we still have more than 30 hours to maturity, we use ONE_HOUR candlesticks to get variance,
    //if not, we switch to FIVE_MIN and use, if we have more than 30*5min
    //if not, we use ONE_MIN candles
    const secondsToMaturity = maturity - epochInSeconds
    if (secondsToMaturity < 0) {
        throw Error("Illegal internal state, this should never happen")
    }
    const hoursToMaturity = secondsToMaturity / (60 * 60)
    let granularity: CandleGranularity;
    if (hoursToMaturity >= STEP_SAMPLE_SIZE) {
        granularity = CandleGranularity.ONE_HOUR
    } else {
        const fiveMinsToMaturity = secondsToMaturity / (60 * 5)
        if (fiveMinsToMaturity >= STEP_SAMPLE_SIZE) {
            granularity = CandleGranularity.FIVE_MIN
        } else {
            granularity = CandleGranularity.ONE_MIN
        }
    }
    console.log(`Chosen granularity was ${granularity.toString()}`)
    let candles = await getCandlesticksFrom(vaultFactoryAccount.baseAsset.toString(), epochInSeconds - secondsToMaturity, granularity)
    const conn = program.provider.connection
    const mintBaseAsset = await token.getMint(conn, vaultFactoryAccount.baseAsset)
    const mintQuoteAsset = await token.getMint(conn, vaultFactoryAccount.quoteAsset)
    //console.log("First candle:")
    //console.log(candles[0])

    //sorting by decreasing startTime
    candles.sort((a, b) => (a.startTime > b.startTime ? -1 : 1))

    //checking if candles are unexpectedly too old
    if (epochInSeconds - candles[0].startTime > MAX_STEPS_TO_TOO_OLD*granularityToSeconds(granularity)) {
        throw Error(`Cannot trust datafeed, candle stick data is too old. Please try again later. Last startTime epoch was: ${candles[0].startTime}`)
    }
    //console.log("first 5 candles without duplicates, by decreasing startTime")
    //console.log(candles.slice(0, 5))

    let currentTokenPrice = await tokenLastMinuteCandle(vaultFactoryAccount.baseAsset.toString())


    //console.log(currentTokenPrice)
    //console.log('cdf normal test')
    //console.log(cdfNormal(5, 30, 25))

    const newPrice = computePutOptionFairPrice(currentTokenPrice.close, vaultFactoryAccount.strike.toNumber(), candles, granularity)
    //this is the price of 1 base asset considering decimals of quote asset
    //for instance, as USDC has 6 decimals, should divide by 1000000 to have price in dollars 
    const maturityEpoch = vaultFactoryAccount.maturity.toNumber()
    var d = new Date(0)
    d.setUTCSeconds(maturityEpoch)
    console.log(`The fair price to the right to sell 1 bitcoin for ${vaultFactoryAccount.strike.toNumber()/(10**(mintQuoteAsset.decimals))} dollars at ${d.toUTCString()} is ${newPrice/(10**mintQuoteAsset.decimals)}`)
    const oracleKeyPair = anchor.web3.Keypair.fromSecretKey(Uint8Array.from(ORACLE_KEY))
    let tx = program.methods.oracleUpdatePrice(new anchor.BN(newPrice)).accounts({
        vaultFactoryInfo: vaultFactoryInfo,
        updateTicket: ticketAddress,
        ticketOwner: user,
        initializer: oracleKeyPair.publicKey
    }).signers([oracleKeyPair]).rpc()
    return tx
}

function computePutOptionFairPrice(
    currentPrice: number, 
    strike: number, 
    volatilitySource: any[], //list of candles ordered by decreasing start time
    volatilitySourceGranularity: CandleGranularity,
    riskFreeYearlyRate: number = RISK_FREE_YEARLY_RATE
    ): number {
        console.log(`currentPrice: ${currentPrice}, strike: ${strike}`)
        const r = convertInterest(riskFreeYearlyRate, 360*24*60*60, granularityToSeconds(volatilitySourceGranularity))
        console.log(`r is ${r}`)
        const sigma2 = volatilitySquared(volatilitySource.map(candle => candle.close))
        console.log(`sigma2 is ${sigma2}`)
        const timeSteps = (volatilitySource[0].startTime - volatilitySource[volatilitySource.length - 1].startTime)/granularityToSeconds(volatilitySourceGranularity)
        console.log(`We are using ${timeSteps} timesteps`)
        const denominator = Math.sqrt(sigma2*timeSteps)
        console.log(`Denominator is ${denominator}`)
        const d1 = (Math.log(currentPrice/strike) + (r + sigma2/2)*timeSteps)/denominator
        console.log(`d1 is ${d1}`)
        const d2 = d1 - denominator
        console.log(`d2 is ${d2}`)

        const p = strike*Math.pow(Math.E, -1.0*r*timeSteps)*cdfStdNormal(-1.0*d2) - currentPrice*cdfStdNormal(-1.0*d1)
        return p
}

export const lastKnownPrice = async (
    mint: string
): Promise<TokenPrice> => {
    const lastCandle = await tokenLastMinuteCandle(mint)
    const result = new TokenPrice(lastCandle)
    return result
}

async function tokenLastMinuteCandle(mint: string) {
    const epochInSeconds = Math.floor(Date.now() / 1000);
    const thisMinuteMinusMaxDelay = epochInSeconds - (epochInSeconds % 60) - CURRENT_PRICE_MAX_DELAY_SECONDS

    let resultArray = await getCandlesticksFrom(mint, thisMinuteMinusMaxDelay, CandleGranularity.ONE_MIN);
    if (resultArray.length < 1) {
        throw Error("Could not get reliable last price for mint, please try again in a few minutes")
    }
    let maxStartTime = 0
    let result = undefined
    resultArray.forEach(candle => {
        if (candle.startTime > maxStartTime) {
            result = candle
            maxStartTime = candle.startTime
        }
    });
    return result
}

async function getCandlesticksBetween(mint: string, startTimeEpoch: number, endTimeEpoch: number, granularity: CandleGranularity) {
    const endpoint = "/v0/token/candlesticks"
    if (DEVNET_MODE) {
        mint = devnetMockMintTranslator[mint]
    }
    let postData = {
        "startTime": {
            "operator": "between",
            "greaterThan": startTimeEpoch,
            "lessThan": endTimeEpoch
        },
        "granularity": granularity.toString(),
        "mint": mint
    }
    let result = await axiosInstance.post(
        endpoint,
        postData
    ) 
    var helloCandles = result.data.data
    while(result.data.paginationToken) {
        postData["paginationToken"] = result.data.paginationToken
        let newResult = await axiosInstance.post(endpoint, postData)
        helloCandles = helloCandles.concat(newResult.data.data)
        result = newResult
    }


    // assure that we do not have duplicates, just in case...
    type CandlesByStartTime = {
        [startTime: number]: any;
    }
    const candlesByStartTime: CandlesByStartTime = {};
    helloCandles.forEach(candle => {
        candlesByStartTime[candle.startTime] = candle
    });
    const returnedResult = Object.values(candlesByStartTime)

    return returnedResult

}

async function getCandlesticksFrom(mint: string, startTimeEpoch: number, granularity: CandleGranularity) {
    const endpoint = "/v0/token/candlesticks"
    if (DEVNET_MODE) {
        mint = devnetMockMintTranslator[mint]
        console.log("Mint will be translated to ", mint)
    }
    console.log("Called getCandlesticksFrom")
    let postData = {
        "startTime": {
            "operator": ">=",
            "value": startTimeEpoch
        },
        "granularity": granularity.toString(),
        "mint": mint
    }
    let result = await axiosInstance.post(
        endpoint,
        postData
    ) 
    var helloCandles = result.data.data
    while(result.data.paginationToken) {
        postData["paginationToken"] = result.data.paginationToken
        let newResult = await axiosInstance.post(endpoint, postData)
        helloCandles = helloCandles.concat(newResult.data.data)
        result = newResult
    }


    // assure that we do not have duplicates, just in case...
    type CandlesByStartTime = {
        [startTime: number]: any;
    }
    const candlesByStartTime: CandlesByStartTime = {};
    helloCandles.forEach(candle => {
        candlesByStartTime[candle.startTime] = candle
    });
    const returnedResult = Object.values(candlesByStartTime)

    return returnedResult
}

export const _testInitializeOracleAccount = async (connection: anchor.web3.Connection) => {
    const secretKey = Uint8Array.from(ORACLE_KEY)
    const signer = anchor.web3.Keypair.fromSecretKey(secretKey)
    const balance = await connection.getBalance(signer.publicKey)
    console.log("Current oracle balance is", balance / anchor.web3.LAMPORTS_PER_SOL)

    if (balance < anchor.web3.LAMPORTS_PER_SOL) {
        console.log("Airdropping 1 SOL to oracle...")
        const airdropSignature = await connection.requestAirdrop(
            signer.publicKey,
            anchor.web3.LAMPORTS_PER_SOL
        )

        const latestBlockHash = await connection.getLatestBlockhash()

        await connection.confirmTransaction(
            {
                blockhash: latestBlockHash.blockhash,
                lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
                signature: airdropSignature,
            },
            "finalized"
        )

        const newBalance = await connection.getBalance(signer.publicKey)
        console.log("New oracle balance is", newBalance / anchor.web3.LAMPORTS_PER_SOL)
    }

}