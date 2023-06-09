import * as anchor from "@coral-xyz/anchor";
import { AnchorSolhedge } from "../target/types/anchor_solhedge";
import { getUserTicketAccountAddressForVaultFactory } from "./accounts";
import axios from 'axios'
import { cdfStdNormal, convertInterest, volatilitySquared } from "./stats";
import * as token from "@solana/spl-token"
import * as dotenv from "dotenv";

dotenv.config()

const ORACLE_KEY = [173, 200, 109, 11, 190, 65, 138, 51, 173, 27, 103, 62, 80, 143, 80, 89, 208, 134, 120, 55, 24, 150, 182, 249, 188, 107, 24, 73, 82, 133, 13, 249, 125, 80, 225, 215, 197, 38, 132, 128, 90, 96, 137, 231, 45, 60, 249, 165, 142, 68, 15, 175, 252, 121, 192, 200, 171, 55, 5, 47, 191, 201, 205, 209]
const HELLO_MOON_BEARER = process.env.HELLO_MOON_BEARER;
const ANCHOR_FREEZE_SECONDS = 30 * 60;
const STEP_SAMPLE_SIZE = 30;

//should decrease this (e.g. to 5*60) when hello moon give more reliable data
const CURRENT_PRICE_MAX_DELAY_SECONDS = 20*60
const RISK_FREE_YEARLY_RATE = 0.06;
//should decrease this (e.g. to 3) when hello moon give more reliable data
const MAX_STEPS_TO_TOO_OLD = 6;

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

enum CandleGranularity {
    ONE_MIN = "ONE_MIN",
    FIVE_MIN = "FIVE_MIN",
    ONE_HOUR = "ONE_HOUR",
    ONE_DAY = "ONE_DAY",
    ONE_WEEK = "ONE_WEEK"
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

function granularityToSeconds(granularity: CandleGranularity): number {
    switch(granularity) {
        case CandleGranularity.ONE_MIN:
            return 60;
        case CandleGranularity.FIVE_MIN:
            return 5*60;
        case CandleGranularity.ONE_HOUR:
            return 60*60;
        case CandleGranularity.ONE_DAY:
            return 24*60*60;
        case CandleGranularity.ONE_WEEK:
            return 7*24*60*60;
        default:
            throw Error(`Internal error, unknown granularity: ${granularity}`)
    }
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

async function getCandlesticksFrom(mint: string, startTimeEpoch: number, granularity: CandleGranularity) {
    const endpoint = "/v0/token/candlesticks"
    //console.log("Called getCandlesticksFrom")
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