import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { AnchorSolhedge } from "../target/types/anchor_solhedge";
import * as token from "@solana/spl-token"
import NodeWallet from "@coral-xyz/anchor/dist/cjs/nodewallet";
import { printAddressLookupTable, sendTransactionV0, isLocalnet, keyPairFromSecret } from "./util";
import * as dotenv from "dotenv";
import { SnakeMinterDevnet } from "../target/types/snake_minter_devnet";

import { assert, expect } from "chai";
import { 
  MakerCreatePutOptionParams, 
  MakerCreateCallOptionParams,
  getPutOptionVaultFactoryPdaAddress,
  getCallOptionVaultFactoryPdaAddress,
  getPutOptionVaultDerivedPdaAddresses, 
  getPutMakerVaultAssociatedAccountAddress,
  getCallOptionVaultDerivedPdaAddresses,
  getAllMaybeNotMaturedPutFactories,
  getVaultsForPutFactory,
  getUserMakerInfoAllPutVaults,
  getAllPutMakerInfosForVault,
  getUserMakerInfoForPutVault,
  getPutSellersInVault,
  getUserTicketAccountAddressForPutVaultFactory,
  getUserSettleTicketAccountAddressForPutVaultFactory,
  getPutMakerATAs,
  getMakerNextPutOptionVaultIdFromTx,
  getUserTakerInfoForPutVault,
  getPutSellersAsRemainingAccounts,
  getUserTakerInfoAllPutVaults,
  getAllMaybeNotMaturedCallFactories,
  getVaultsForCallFactory,
  getUserMakerInfoAllCallVaults,
  getAllCallMakerInfosForVault,
  getUserMakerInfoForCallVault,
  getUserTicketAccountAddressForCallVaultFactory,
  getCallSellersInVault,

} from "./accounts";
import * as borsh from "borsh";
import { getOraclePubKey, _testInitializeOracleAccount, updatePutOptionFairPrice, lastKnownPrice, updateCallOptionFairPrice } from "./oracle";
import { snakeBTCMintAddr, snakeDollarMintAddr, mintSnakeDollarTo, mintSnakeBTCTo } from "./snake-minter-devnet";
import { oracleAddr, updatePutOptionSettlePrice } from "./oracle";

dotenv.config()

const TEST_PUT_MAKER_KEY = [7,202,200,249,141,19,80,240,20,148,116,158,237,253,235,157,26,157,95,58,241,232,6,221,233,94,248,189,255,95,87,169,170,77,151,133,53,15,237,214,51,0,2,67,60,75,202,138,200,234,155,157,153,141,162,233,83,179,126,125,248,211,212,51]
const TEST_PUT_MAKER2_KEY = [58,214,126,90,15,29,80,114,170,70,234,58,244,144,25,23,110,1,6,19,176,12,232,59,55,64,56,53,60,187,246,157,140,117,187,255,239,135,134,192,94,254,53,137,53,27,99,244,218,86,207,59,22,189,242,164,155,104,68,250,161,179,108,4]

const TEST_PUT_TAKER_KEY = [198,219,91,244,252,118,0,25,83,232,178,61,51,196,168,151,77,1,142,9,164,80,29,63,76,216,213,85,99,185,71,113,36,61,101,115,203,92,102,70,200,37,98,228,234,240,155,7,144,0,244,71,236,104,22,131,143,216,47,244,151,205,246,245]

const TEST_CALL_TAKER_KEY = [34,70,126,122,54,85,192,254,177,96,78,120,138,157,162,99,28,229,168,9,218,245,12,223,5,123,110,251,146,64,80,78,38,119,242,115,10,183,83,73,233,36,67,234,180,208,112,249,135,92,67,180,230,128,155,183,154,4,12,21,2,232,205,209]

const TEST_CALL_MAKER_KEY = [223,213,193,53,156,60,130,254,205,49,112,44,52,72,232,5,125,35,122,49,199,54,17,93,178,243,206,107,167,174,251,89,23,1,101,73,149,218,109,106,30,26,112,132,101,81,192,248,142,207,82,231,106,25,255,162,87,37,185,91,158,112,242,210]

const TEST_CALL_MAKER2_KEY = [135,38,149,130,53,194,212,234,171,1,139,229,29,85,32,193,243,6,149,195,225,139,233,179,6,29,170,121,87,229,88,48,67,213,147,114,107,206,212,88,39,107,14,98,185,48,251,248,100,125,48,148,207,88,233,166,7,185,86,43,73,183,160,22]

// The corresponding pubkey of this key is what we should put in pyutil/replaceMint.py to generate the mocks USDC and WBTC
const TEST_MOCK_MINTER_KEY = [109,3,86,101,96,42,254,204,98,232,34,172,105,37,112,24,223,194,66,133,2,105,54,228,54,97,90,111,253,35,245,73,93,83,136,36,51,237,111,8,250,149,126,98,135,211,138,191,207,116,66,179,204,231,147,190,217,190,220,93,181,102,164,238]

const TEST_PROTOCOL_FEES_KEY = JSON.parse(process.env.DEVNET_PROTOCOL_FEES_KEY) as number[]

const DEVNET_PROTOCOL_FEES_PUBKEY = process.env.DEVNET_PROTOCOL_FEES_PUBKEY

const protocolFeesAddr = new anchor.web3.PublicKey(DEVNET_PROTOCOL_FEES_PUBKEY)

// Should be the same as FREEZE_SECONDS in anchor-solhedge/lib.rs
const FREEZE_SECONDS = 30*60

async function fundPeerIfNeeded(
  payer: anchor.web3.Keypair, 
  peer: anchor.web3.PublicKey,
  connection: anchor.web3.Connection
) {
  const balance = await connection.getBalance(peer)
  console.log(`Current balance for peer ${peer.toString()} is `, balance / anchor.web3.LAMPORTS_PER_SOL)
  if (balance < 0.1 * anchor.web3.LAMPORTS_PER_SOL) {
    console.log("Funding peer with 0.1 SOL")
    const transferTransaction = new anchor.web3.Transaction().add(
      anchor.web3.SystemProgram.transfer({
        fromPubkey: payer.publicKey,
        toPubkey: peer,
        lamports: 0.1 * anchor.web3.LAMPORTS_PER_SOL
      })
    )
    await anchor.web3.sendAndConfirmTransaction(connection, transferTransaction, [payer], {commitment: "finalized"})
  }
}

async function airdropSolIfNeeded(
  signer: anchor.web3.Keypair,
  connection: anchor.web3.Connection
) {
  const balance = await connection.getBalance(signer.publicKey)
  console.log("Current balance is", balance / anchor.web3.LAMPORTS_PER_SOL)

  if (balance < anchor.web3.LAMPORTS_PER_SOL) {
    console.log("Airdropping 1 SOL...")
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
    console.log("New balance is", newBalance / anchor.web3.LAMPORTS_PER_SOL)
  }
}

export const createTokenAccount = async (
  connection: anchor.web3.Connection,
  payer: anchor.web3.Keypair,
  mint: anchor.web3.PublicKey,
  owner: anchor.web3.PublicKey
) => {
  const tokenAccount = await token.getOrCreateAssociatedTokenAccount(
      connection,
      payer,
      mint,
      owner
  )
  
  // console.log(
  //     `Token Account: ${tokenAccount.address}`
  // )

  return tokenAccount
}


async function mintTokens(
  connection: anchor.web3.Connection,
  payer: anchor.web3.Keypair,
  mint: anchor.web3.PublicKey,
  destination: anchor.web3.PublicKey,
  authority: anchor.web3.Keypair,
  amount: number
) {
  const mintInfo = await token.getMint(connection, mint)

  const transactionSignature = await token.mintTo(
    connection,
    payer,
    mint,
    destination,
    authority,
    amount * 10 ** mintInfo.decimals
  )

  console.log(
    `Mint Token Transaction: ${transactionSignature}`
  )
}

async function getTokenBalance(
  conn: anchor.web3.Connection,
  payer: anchor.web3.Keypair,
  mintAddr: anchor.web3.PublicKey,
  user: anchor.web3.PublicKey
) {
  const userATA = await createTokenAccount(conn, payer, mintAddr, user);
  const balance = Number(userATA.amount)
  return balance
}

describe("anchor-solhedge-devnet", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const DEVNET_DEVEL_KEY = JSON.parse(process.env.PRIVATE_KEY) as number[]
  const DEVNET_PUTMAKER1_KEY = JSON.parse(process.env.DEVNET_PUTMAKER1_KEY) as number[]
  const DEVNET_PUTMAKER2_KEY = JSON.parse(process.env.DEVNET_PUTMAKER2_KEY) as number[]
  const DEVNET_PUTTAKER_KEY = JSON.parse(process.env.DEVNET_PUTTAKER_KEY) as number[]

  const devnetPayerKeypair = keyPairFromSecret(DEVNET_DEVEL_KEY)
  const putMaker1Keypair = keyPairFromSecret(DEVNET_PUTMAKER1_KEY)
  const putMaker2Keypair = keyPairFromSecret(DEVNET_PUTMAKER2_KEY)
  const putTakerKeypair = keyPairFromSecret(DEVNET_PUTTAKER_KEY)

  const program = anchor.workspace.AnchorSolhedge as Program<AnchorSolhedge>;    

  if (!isLocalnet(anchor.getProvider().connection)) {
    console.log("anchor-solhedge.ts devnet tests starting...")

    before(
      "Getting some SOL for devnet payer and his pals, if needed",
      async () => {
        const lamportTransfers = [
          airdropSolIfNeeded(
            devnetPayerKeypair,
            anchor.getProvider().connection
          ),
          fundPeerIfNeeded(devnetPayerKeypair, putMaker1Keypair.publicKey, anchor.getProvider().connection),
          fundPeerIfNeeded(devnetPayerKeypair, putMaker2Keypair.publicKey, anchor.getProvider().connection),
          fundPeerIfNeeded(devnetPayerKeypair, oracleAddr, anchor.getProvider().connection),
          fundPeerIfNeeded(devnetPayerKeypair, protocolFeesAddr, anchor.getProvider().connection),
          fundPeerIfNeeded(devnetPayerKeypair, putTakerKeypair.publicKey, anchor.getProvider().connection)
        ]
        await Promise.all(lamportTransfers)
      } 
    )
      
    it("Is initialized!", async () => {
      // Add your test here.
      const tx = await program.methods.initialize().rpc()
      console.log("Your transaction signature", tx);
    });

    it(`Minting 500 SnakeDollars to ${putMaker1Keypair.publicKey} if his balance is < 500`, async () => {
      let balance = await getTokenBalance(anchor.getProvider().connection, devnetPayerKeypair, snakeDollarMintAddr, putMaker1Keypair.publicKey)
      const mint = await token.getMint(anchor.getProvider().connection, snakeDollarMintAddr)
      balance /= 10**mint.decimals
      console.log(`${putMaker1Keypair.publicKey.toString()} SnakeDollar balance is ${balance}`)
      if (balance < 500) {
        const snakeMinterProg = anchor.workspace.SnakeMinterDevnet as Program<SnakeMinterDevnet>;
        const tx = await mintSnakeDollarTo(snakeMinterProg, putMaker1Keypair)
        console.log('Mint tx: ', tx)
      }
    });

    it(`Minting 500 SnakeDollars to ${putMaker2Keypair.publicKey} if his balance is < 500`, async () => {
      let balance = await getTokenBalance(anchor.getProvider().connection, devnetPayerKeypair, snakeDollarMintAddr, putMaker2Keypair.publicKey)
      const mint = await token.getMint(anchor.getProvider().connection, snakeDollarMintAddr)
      balance /= 10**mint.decimals

      console.log(`${putMaker2Keypair.publicKey.toString()} SnakeDollar balance is ${balance}`)
      if (balance < 500) {
        const snakeMinterProg = anchor.workspace.SnakeMinterDevnet as Program<SnakeMinterDevnet>;
        const tx = await mintSnakeDollarTo(snakeMinterProg, putMaker2Keypair)
        console.log('Mint tx: ', tx)
      }
    });

    it(`Minting 500 SnakeDollars to taker ${putTakerKeypair.publicKey} if his balance is < 500, he needs dollars to pay for option premium`, async () => {
      let balance = await getTokenBalance(anchor.getProvider().connection, devnetPayerKeypair, snakeDollarMintAddr, putTakerKeypair.publicKey)
      const mint = await token.getMint(anchor.getProvider().connection, snakeDollarMintAddr)
      balance /= 10**mint.decimals

      console.log(`${putTakerKeypair.publicKey.toString()} SnakeDollar balance is ${balance}`)
      if (balance < 500) {
        const snakeMinterProg = anchor.workspace.SnakeMinterDevnet as Program<SnakeMinterDevnet>;
        const tx = await mintSnakeDollarTo(snakeMinterProg, putTakerKeypair)
        console.log('Mint tx: ', tx)
      }
    });


    it(`Minting 0.02 SnakeBTC to ${putTakerKeypair.publicKey} if his balance is < 0.02`, async () => {
      let balance = await getTokenBalance(anchor.getProvider().connection, devnetPayerKeypair, snakeBTCMintAddr, putTakerKeypair.publicKey)
      const mint = await token.getMint(anchor.getProvider().connection, snakeBTCMintAddr)
      balance /= 10**mint.decimals

      console.log(`${putTakerKeypair.publicKey.toString()} SnakeBTC balance is ${balance}`)
      if (balance < 0.02) {
        const snakeMinterProg = anchor.workspace.SnakeMinterDevnet as Program<SnakeMinterDevnet>;
        const tx = await mintSnakeBTCTo(snakeMinterProg, putTakerKeypair)
        console.log('Mint tx: ', tx)
      }
    });

    xit(`Now ${putMaker1Keypair.publicKey} is creating a Vault Factory and a Vault inside it as a PutMaker`, async () => {
      console.log(">>>>> CALLING LAST KNOWN PRICE")
      const btcPrice = await lastKnownPrice(snakeBTCMintAddr.toBase58()) //wBTC
      console.log("Last known price for wBTC is ", btcPrice.price)

      const currEpoch = Math.floor(Date.now()/1000)
      const oneDay = currEpoch + (24*60*60)
      const myStrike = Math.round(btcPrice.price*0.99)
      console.log(`I will offer 10 put options of 0.001 bitcoins each, at strike ${myStrike} with maturity 24 hours from now`)
      const vaultParams = new MakerCreatePutOptionParams(
        {
          maturity: new anchor.BN(oneDay),
          strike: new anchor.BN(myStrike),
          //lotSize is in 10^lot_size
          lotSize: -3,
          maxMakers: 100,
          maxTakers: 100,
          numLotsToSell: new anchor.BN(10),
          premiumLimit: new anchor.BN(0)  
        }
      )
      const putOptionVaultFactoryAddress = await getPutOptionVaultFactoryPdaAddress(program, snakeBTCMintAddr, snakeDollarMintAddr, vaultParams.maturity, vaultParams.strike)

      const tx = await program.methods.makerNextPutOptionVaultId(vaultParams).accounts({
        initializer: putMaker1Keypair.publicKey,
        vaultFactoryInfo: putOptionVaultFactoryAddress,
        baseAssetMint: snakeBTCMintAddr,
        quoteAssetMint: snakeDollarMintAddr
      }).signers([putMaker1Keypair]).rpc({ commitment: "confirmed" })

      console.log("Transaction for getting next VaultId is ", tx)

      const vaultNumber = await getMakerNextPutOptionVaultIdFromTx(program, anchor.getProvider().connection, tx)

      const {
        putOptionVaultAddress, 
        vaultBaseAssetTreasury, 
        vaultQuoteAssetTreasury
      } = await getPutOptionVaultDerivedPdaAddresses(program, putOptionVaultFactoryAddress, snakeBTCMintAddr, snakeDollarMintAddr, vaultNumber)

      const putMaker1SnakeDollarATA = await token.getOrCreateAssociatedTokenAccount(
        anchor.getProvider().connection,
        putMaker1Keypair,
        snakeDollarMintAddr,
        putMaker1Keypair.publicKey
      )
  
      var tx2 = await program.methods.makerCreatePutOptionVault(vaultParams, vaultNumber).accounts({
        initializer: putMaker1Keypair.publicKey,
        vaultFactoryInfo: putOptionVaultFactoryAddress,
        vaultInfo: putOptionVaultAddress,
        vaultBaseAssetTreasury: vaultBaseAssetTreasury,
        vaultQuoteAssetTreasury: vaultQuoteAssetTreasury,
        baseAssetMint: snakeBTCMintAddr,
        quoteAssetMint: snakeDollarMintAddr,
        makerQuoteAssetAccount: putMaker1SnakeDollarATA.address,
      }).signers([putMaker1Keypair]).rpc()
      console.log("Transaction for creating PutOptionVault is ", tx2)

      const vaultFactories = await getAllMaybeNotMaturedPutFactories(program)
      let myFactory = undefined
      for (let vaultFactory of vaultFactories) {
        if (
          vaultFactory.account.maturity.toNumber() == vaultParams.maturity.toNumber() &&
          vaultFactory.account.baseAsset.toString() == snakeBTCMintAddr.toString() &&
          vaultFactory.account.quoteAsset.toString() == snakeDollarMintAddr.toString() &&
          vaultFactory.account.strike.toNumber() == vaultParams.strike.toNumber()
        ) {
          myFactory = vaultFactory
          break
        }
      }
      assert.notEqual(myFactory, undefined)
      assert.equal(myFactory.account.isInitialized, true)
      assert.equal(myFactory.account.matured, false)
      const factoryKey = myFactory.publicKey
  
      const vaultsForFactory = await getVaultsForPutFactory(program, factoryKey)
      assert.equal(vaultsForFactory[0].account.maxMakers, vaultParams.maxMakers)
      assert.equal(vaultsForFactory[0].account.maxTakers, vaultParams.maxTakers)
  
      const userInfoInVault = await getUserMakerInfoAllPutVaults(program, putMaker1Keypair.publicKey)
      assert.equal(userInfoInVault[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
  
      const makerInfos = await getAllPutMakerInfosForVault(program, vaultsForFactory[0].publicKey)
      assert.equal(makerInfos[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
  
      const makerInfoForVault = await getUserMakerInfoForPutVault(program, vaultsForFactory[0].publicKey, putMaker1Keypair.publicKey)
      assert.equal(makerInfoForVault[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
    });

    xit(`Now a second Putmaker, ${putMaker2Keypair.publicKey} will try to enter existing PutOptionVaults`, async () => {
      const vaultFactories = await getAllMaybeNotMaturedPutFactories(program)
      console.log('Number of maybe not matured factories: ', vaultFactories.length)
      for (let vaultFactory of vaultFactories) {
        const maturity = vaultFactory.account.maturity.toNumber()
        console.log(`Maturity of VaultFactory ${vaultFactory.publicKey} is ${maturity}`)
        if (
          vaultFactory.account.quoteAsset.toString() == snakeDollarMintAddr.toString() && 
          maturity > (Math.floor(Date.now()/1000) + FREEZE_SECONDS + 60)
        ) {
            // will only try to enter if there is at least 1 minute to freeze time
          console.log("Getting vaults for fault factory ", vaultFactory.publicKey.toString())
          const vaults = await getVaultsForPutFactory(program, vaultFactory.publicKey)
          for (let vault of vaults) {
            if (vault.account.isMakersFull) continue
            if ((await getUserMakerInfoForPutVault(program, vault.publicKey, putMaker2Keypair.publicKey)).length > 0) {
              continue
            }
            const minEntry = Math.ceil((10**vault.account.lotSize)*vaultFactory.account.strike.toNumber())
            const myBalance = await getTokenBalance(program.provider.connection, putMaker2Keypair, snakeDollarMintAddr, putMaker2Keypair.publicKey)
            const {
              putOptionVaultAddress, 
              vaultBaseAssetTreasury, 
              vaultQuoteAssetTreasury
            } = await getPutOptionVaultDerivedPdaAddresses(program, vaultFactory.publicKey, vaultFactory.account.baseAsset, vaultFactory.account.quoteAsset, vault.account.ord)
      
            if (myBalance >= minEntry) {
              const numLots = Math.floor(myBalance/minEntry)
              let tx3 = await program.methods.makerEnterPutOptionVault(new anchor.BN(numLots), new anchor.BN(0)).accounts({
                initializer: putMaker2Keypair.publicKey,
                vaultFactoryInfo: vaultFactory.publicKey,
                vaultInfo: vault.publicKey,
                vaultQuoteAssetTreasury: vaultQuoteAssetTreasury,
                baseAssetMint: vaultFactory.account.baseAsset,
                quoteAssetMint: vaultFactory.account.quoteAsset,
                makerQuoteAssetAccount: token.getAssociatedTokenAddressSync(vaultFactory.account.quoteAsset, putMaker2Keypair.publicKey, false),
              }).signers([putMaker2Keypair]).rpc()
              console.log(`Transaction for ${putMaker2Keypair.publicKey} entering vault ${vault.publicKey} is`, tx3)
              let maker2InfoForVault = await getUserMakerInfoForPutVault(program, vault.publicKey, putMaker2Keypair.publicKey)
              const quoteAssetQty = maker2InfoForVault[0].account.quoteAssetQty.toNumber()
              assert.isTrue(quoteAssetQty > 0)
        
            }
          }
        }
      }
    });

    xit(`Now put maker ${putMaker2Keypair.publicKey} will play with adjusting his position on vaults`, async () => {
      const vaultFactories = await getAllMaybeNotMaturedPutFactories(program)
      console.log(`${putMaker2Keypair.publicKey} will look at ${vaultFactories.length} maybe not matured factories: `)
      for (let vaultFactory of vaultFactories) { 
        const maturity = vaultFactory.account.maturity.toNumber()
        console.log(`Maturity of VaultFactory ${vaultFactory.publicKey} is ${maturity}`)
        if (
          vaultFactory.account.quoteAsset.toString() == snakeDollarMintAddr.toString() && 
          maturity > (Math.floor(Date.now()/1000) + FREEZE_SECONDS + 60)
        ) {
            // will only try to enter if there is at least 1 minute to freeze time
          console.log("Getting vaults for fault factory ", vaultFactory.publicKey.toString())
          const vaults = await getVaultsForPutFactory(program, vaultFactory.publicKey)

          for (let vault of vaults) {
            let maker2InfoForVault = await getUserMakerInfoForPutVault(program, vault.publicKey, putMaker2Keypair.publicKey)
            if (maker2InfoForVault.length > 0) { // putmaker is in the vault
              const notSoldQty = maker2InfoForVault[0].account.quoteAssetQty.toNumber() - maker2InfoForVault[0].account.volumeSold.toNumber()
              const lotPrice = vaultFactory.account.strike.toNumber()*(10**vault.account.lotSize)
              if (notSoldQty > 0) {
                const {
                  putOptionVaultAddress, 
                  vaultBaseAssetTreasury, 
                  vaultQuoteAssetTreasury
                } = await getPutOptionVaultDerivedPdaAddresses(program, vaultFactory.publicKey, vaultFactory.account.baseAsset, vaultFactory.account.quoteAsset, vault.account.ord)
    
                const newQtyLots = maker2InfoForVault[0].account.volumeSold.toNumber()/lotPrice
                let tx4 = await program.methods.makerAdjustPositionPutOptionVault(new anchor.BN(newQtyLots), new anchor.BN(0)).accounts({        
                  initializer: putMaker2Keypair.publicKey,
                  vaultFactoryInfo: vaultFactory.publicKey,
                  vaultInfo: vault.publicKey,
                  vaultQuoteAssetTreasury: vaultQuoteAssetTreasury,
                  putOptionMakerInfo: maker2InfoForVault[0].publicKey,
                  baseAssetMint: vaultFactory.account.baseAsset,
                  quoteAssetMint: vaultFactory.account.quoteAsset,
                  makerQuoteAssetAccount: token.getAssociatedTokenAddressSync(vaultFactory.account.quoteAsset, putMaker2Keypair.publicKey, false)
                }).signers([putMaker2Keypair]).rpc()
                maker2InfoForVault = await getUserMakerInfoForPutVault(program, vault.publicKey, putMaker2Keypair.publicKey)
                assert.equal(maker2InfoForVault[0].account.quoteAssetQty.toNumber(), newQtyLots*lotPrice)
          
              }
            }
          }

        }
      }

    });

    xit("A PutTaker will now try to find vaults where he can enter", async () => {
      const slippageTolerance = 0.05      
      const vaultFactories = await getAllMaybeNotMaturedPutFactories(program)
      console.log(`PutTaker ${putTakerKeypair.publicKey} will look at ${vaultFactories.length} maybe not matured factories: `)
      for (let vaultFactory of vaultFactories) { 
        const maturity = vaultFactory.account.maturity.toNumber()
        console.log(`Maturity of VaultFactory ${vaultFactory.publicKey} is ${maturity}`)
        if (
          vaultFactory.account.baseAsset.toString() == snakeBTCMintAddr.toString() && 
          maturity > (Math.floor(Date.now()/1000) + FREEZE_SECONDS + 60)
        ) {
            // will only try to enter if there is at least 1 minute to freeze time
          console.log("Getting vaults for fault factory ", vaultFactory.publicKey.toString())
          const vaults = await getVaultsForPutFactory(program, vaultFactory.publicKey)

          for (let vault of vaults) {
            if (!vault.account.isTakersFull) {
              const myTakerInfo = await getUserTakerInfoForPutVault(program, vault.publicKey, putTakerKeypair.publicKey)
              if (myTakerInfo.length > 0) {
                continue
              }
              console.log(`PutTaker ${putTakerKeypair.publicKey} is not in vault ${vault.publicKey} will try to enter`)
              const ticketAddress = await getUserTicketAccountAddressForPutVaultFactory(program, vault.account.factoryVault, putTakerKeypair.publicKey)
              const oracleAddress = getOraclePubKey()
              console.log("Put taker before paying oracle SOL balance is", await anchor.getProvider().connection.getBalance(putTakerKeypair.publicKey)/ anchor.web3.LAMPORTS_PER_SOL)
              console.log("Oracle SOL balance is", await anchor.getProvider().connection.getBalance(oracleAddress)/ anchor.web3.LAMPORTS_PER_SOL)
        
              let tx6 = await program.methods.genUpdatePutOptionFairPriceTicket().accounts({
                vaultFactoryInfo: vault.account.factoryVault,
                initializer: putTakerKeypair.publicKey,
                oracleWallet: oracleAddress,
                putOptionFairPriceTicket: ticketAddress
              }).signers([putTakerKeypair]).rpc()

              console.log("Put taker after paying oracle SOL balance is", await anchor.getProvider().connection.getBalance(putTakerKeypair.publicKey)/ anchor.web3.LAMPORTS_PER_SOL)    
              console.log("Oracle SOL balance is", await anchor.getProvider().connection.getBalance(oracleAddress)/ anchor.web3.LAMPORTS_PER_SOL)
                
              let tx7 = await updatePutOptionFairPrice(program, vault.account.factoryVault, putTakerKeypair.publicKey)

              console.log("Oracle SOL balance after updating fair price is", await anchor.getProvider().connection.getBalance(oracleAddress)/ anchor.web3.LAMPORTS_PER_SOL)
              console.log("Put taker after oracle using ticket SOL balance is", await anchor.getProvider().connection.getBalance(putTakerKeypair.publicKey)/ anchor.web3.LAMPORTS_PER_SOL)        
              let updatedVaultFactory = await program.account.putOptionVaultFactoryInfo.fetch(vault.account.factoryVault)
              console.log("Updated put option fair price is", updatedVaultFactory.lastFairPrice.toNumber())
              const sellers = await getPutSellersInVault(program, vault.publicKey, updatedVaultFactory.lastFairPrice.toNumber(), slippageTolerance)

              const vaultPendingSell = vault.account.makersTotalPendingSell.toNumber()
              console.log('Total quote asset volume pending sell ', vaultPendingSell)
              const lotQuoteAssetValue = updatedVaultFactory.strike.toNumber()*(10**vault.account.lotSize)
              const lotsOnSell = Math.floor(vaultPendingSell/lotQuoteAssetValue)
              console.log(`There are at most ${lotsOnSell} on sell in vault ${vault.publicKey}`)

              let balanceBaseAsset = await getTokenBalance(anchor.getProvider().connection, devnetPayerKeypair, snakeBTCMintAddr, putTakerKeypair.publicKey)
              const btcLamports = balanceBaseAsset
              const mint = await token.getMint(anchor.getProvider().connection, snakeBTCMintAddr)
              balanceBaseAsset /= 10**mint.decimals
              console.log(`${putTakerKeypair.publicKey.toString()} SnakeBTC balance is ${balanceBaseAsset}`)

              // value of base assets that putTaker has in quote asset lamports
              const valueAtStrike = balanceBaseAsset*vaultFactory.account.strike.toNumber()
              const maxLotsToBuy = Math.floor(valueAtStrike/lotQuoteAssetValue)
              let balanceQuoteAsset = await getTokenBalance(anchor.getProvider().connection, devnetPayerKeypair, snakeDollarMintAddr, putTakerKeypair.publicKey)
              console.log(`Puttaker has ${balanceQuoteAsset} in quote asset lamports`)
              const lotPremium = updatedVaultFactory.lastFairPrice.toNumber()*(10**vault.account.lotSize)
              const lotsQuoteAssetCanBuy = balanceQuoteAsset/lotPremium
              const lotsToBuy = Math.min(maxLotsToBuy, lotsQuoteAssetCanBuy)
              console.log(`Puttaker ${putTakerKeypair.publicKey} will try to buy ${lotsToBuy} lots`)
              const remainingAccounts = await getPutSellersAsRemainingAccounts(lotsToBuy, program, sellers)
              const protocolFeesUSDCATA = await createTokenAccount(anchor.getProvider().connection, devnetPayerKeypair, snakeDollarMintAddr, protocolFeesAddr)
              const myMaxPrice = Math.floor(updatedVaultFactory.lastFairPrice.toNumber()*1.05)
              

              let tx8 = await program.methods.takerBuyLotsPutOptionVault(
                new anchor.BN(myMaxPrice), 
                new anchor.BN(lotsToBuy), 
                new anchor.BN(btcLamports)).accounts({
                  baseAssetMint: snakeBTCMintAddr,
                  quoteAssetMint: snakeDollarMintAddr,
                  initializer: putTakerKeypair.publicKey,
                  protocolQuoteAssetTreasury: protocolFeesUSDCATA.address,
                  frontendQuoteAssetTreasury: protocolFeesUSDCATA.address, //also sending frontend share to protocol in this test
                  takerBaseAssetAccount: token.getAssociatedTokenAddressSync(snakeBTCMintAddr, putTakerKeypair.publicKey, false),
                  takerQuoteAssetAccount: token.getAssociatedTokenAddressSync(snakeDollarMintAddr, putTakerKeypair.publicKey, false),
                  vaultFactoryInfo: vaultFactory.publicKey,
                  vaultInfo: vault.publicKey,
                  vaultBaseAssetTreasury: token.getAssociatedTokenAddressSync(snakeBTCMintAddr, vault.publicKey, true),
                }).remainingAccounts(
                  remainingAccounts
                ).signers([putTakerKeypair]).rpc()
                console.log(`Transaction id where Puttaker ${putTakerKeypair.publicKey} entered vault ${vault.publicKey}: `, tx8)
            }
          }
        }
      }
    });

    it(`Now PutMaker ${putMaker1Keypair.publicKey} will ask oracle to settle price on matured vaults he is in`, async () => {
      const makerInfosAllVaults = await getUserMakerInfoAllPutVaults(program, putMaker1Keypair.publicKey)
      let currEpoch = Math.floor(Date.now()/1000)

      for (const makerInfo of makerInfosAllVaults) {
        const vaultAddr = makerInfo.account.putOptionVault
        const vaultInfo = await program.account.putOptionVaultInfo.fetch(vaultAddr)
        const vaultFactoryInfo = await program.account.putOptionVaultFactoryInfo.fetch(vaultInfo.factoryVault)
        if (!vaultFactoryInfo.matured && vaultFactoryInfo.maturity.toNumber() < currEpoch) {
          console.log(`Vault factory ${vaultInfo.factoryVault} has matured, will now ask oracle to settle price`)
          const ticketAddress = await getUserSettleTicketAccountAddressForPutVaultFactory(program, vaultInfo.factoryVault, putMaker1Keypair.publicKey)
          let ticketAccount = undefined
          try {
            ticketAccount = await program.account.putOptionSettlePriceTicketInfo.fetch(ticketAddress)
            console.log('TICKET ACCOUNT IS')
            console.log(ticketAccount)  
          } catch(e) {
            console.log('No previous ticket for settling this vault factory found for this user')
          }
          
          if (ticketAccount?.isUsed == undefined) {
            const oracleAddress = getOraclePubKey()
            let tx6 = await program.methods.genSettlePutOptionPriceTicket().accounts({
              vaultFactoryInfo: vaultInfo.factoryVault,
              initializer: putMaker1Keypair.publicKey,
              oracleWallet: oracleAddress,
              putOptionSettlePriceTicket: ticketAddress
            }).signers([putMaker1Keypair]).rpc()
            console.log('Transaction that generated settle price ticket is ', tx6)  
          }
          const tx7 = await updatePutOptionSettlePrice(program, vaultInfo.factoryVault, putMaker1Keypair.publicKey)
          console.log('Transaction where oracle updated settle price for vault factory was ', tx7)
        }
      }
    });

    it("Now put makers will get out of the settled options they are in", async () => {
      const putMakers = [putMaker1Keypair, putMaker2Keypair]
      for (const putMaker of putMakers) {
        const makerInfosAllVaults = await getUserMakerInfoAllPutVaults(program, putMaker.publicKey)
        for (const makerInfo of makerInfosAllVaults) { 
          const vaultAddr = makerInfo.account.putOptionVault
          const vaultInfo = await program.account.putOptionVaultInfo.fetch(vaultAddr)
          const vaultFactoryInfo = await program.account.putOptionVaultFactoryInfo.fetch(vaultInfo.factoryVault)
          const baseAssetATAAddr = await createTokenAccount(anchor.getProvider().connection, devnetPayerKeypair, snakeBTCMintAddr, putMaker.publicKey)
          if (vaultFactoryInfo.matured && !makerInfo.account.isSettled) {
            console.log(`Put maker ${putMaker.publicKey} will get of option vault ${vaultAddr}`)
            let tx = await program.methods.makerSettlePutOption().accounts({
              baseAssetMint: vaultFactoryInfo.baseAsset,
              initializer: putMaker.publicKey,
              makerBaseAssetAccount: baseAssetATAAddr.address,
              makerQuoteAssetAccount: token.getAssociatedTokenAddressSync(snakeDollarMintAddr, putMaker.publicKey, false),
              putOptionMakerInfo: makerInfo.publicKey,
              quoteAssetMint: vaultFactoryInfo.quoteAsset,
              vaultBaseAssetTreasury: token.getAssociatedTokenAddressSync(snakeBTCMintAddr, vaultAddr, true),
              vaultFactoryInfo: vaultInfo.factoryVault,
              vaultInfo: vaultAddr,
              vaultQuoteAssetTreasury: token.getAssociatedTokenAddressSync(snakeDollarMintAddr, vaultAddr, true)
            }).signers([putMaker]).rpc()
            console.log("Transaction id that settled option for maker: ", tx)
          }
        }
        
      }
    });
    
    it("Now put takers will get out of the settled options they are in", async () => {
      const putTakers = [putTakerKeypair]
      for (const putTaker of putTakers) {
        const takerInfoAllVaults = await getUserTakerInfoAllPutVaults(program, putTaker.publicKey)
        for (const takerInfo of takerInfoAllVaults) {
          const vaultAddr = takerInfo.account.putOptionVault
          const vaultInfo = await program.account.putOptionVaultInfo.fetch(vaultAddr)
          const vaultFactoryInfo = await program.account.putOptionVaultFactoryInfo.fetch(vaultInfo.factoryVault)
          const takerQuoteAssetATA = await createTokenAccount(anchor.getProvider().connection, devnetPayerKeypair, snakeDollarMintAddr, putTaker.publicKey)
          if (vaultFactoryInfo.matured && !takerInfo.account.isSettled) {
            console.log(`Put taker ${putTaker.publicKey} will get of option vault ${vaultAddr}`)
            let tx = await program.methods.takerSettlePutOption().accounts({
              baseAssetMint: vaultFactoryInfo.baseAsset,
              initializer: putTaker.publicKey,
              putOptionTakerInfo: takerInfo.publicKey,
              quoteAssetMint: vaultFactoryInfo.quoteAsset,
              takerBaseAssetAccount: token.getAssociatedTokenAddressSync(snakeBTCMintAddr, putTaker.publicKey, false),
              takerQuoteAssetAccount: takerQuoteAssetATA.address,
              vaultBaseAssetTreasury: token.getAssociatedTokenAddressSync(snakeBTCMintAddr, vaultAddr, true),
              vaultFactoryInfo: vaultInfo.factoryVault,
              vaultInfo: vaultAddr,
              vaultQuoteAssetTreasury: token.getAssociatedTokenAddressSync(snakeDollarMintAddr, vaultAddr, true)
            }).signers([putTaker]).rpc()
            console.log("Transaction id that settled option for taker: ", tx)
          }
        }
      }
    });
    
  }
})

describe("anchor-solhedge-localnet", () => {
  //console.log(anchor.AnchorProvider.env())
  
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());
  if (isLocalnet(anchor.getProvider().connection)) {
    console.log('YES! This is localnet!')

    const program = anchor.workspace.AnchorSolhedge as Program<AnchorSolhedge>;

    const minterKeypair = keyPairFromSecret(TEST_MOCK_MINTER_KEY)
    const putMakerKeypair = keyPairFromSecret(TEST_PUT_MAKER_KEY)
    const putMaker2Keypair = keyPairFromSecret(TEST_PUT_MAKER2_KEY)
    const callMakerKeypair = keyPairFromSecret(TEST_CALL_MAKER_KEY)
    const callMaker2Keypair = keyPairFromSecret(TEST_CALL_MAKER2_KEY)

  
    const putTakerKeypair = keyPairFromSecret(TEST_PUT_TAKER_KEY)
    const callTakerKeypair = keyPairFromSecret(TEST_CALL_TAKER_KEY)
    const protocolFeesKeypair = keyPairFromSecret(TEST_PROTOCOL_FEES_KEY)
  
    const usdcToken = new anchor.web3.PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
    const wormholeBTCToken = new anchor.web3.PublicKey("3NZ9JMVBmGAqocybic2c7LQCJScmgsAZ6vQqTDzcqmJh")
  
    const confirmOptions: anchor.web3.ConfirmOptions = { commitment: "confirmed" };
  
    const getReturnLog = (confirmedTransaction) => {
      const prefix = "Program return: ";
      let log = confirmedTransaction.meta.logMessages.find((log) =>
        log.startsWith(prefix)
      );
      log = log.slice(prefix.length);
      const [key, data] = log.split(" ", 2);
      const buffer = Buffer.from(data, "base64");
      return [key, data, buffer];
    };  
  
    before(
      "Getting some SOL for minter and put maker, if needed",
      async () => {
        const airdrops = [
          airdropSolIfNeeded(
            minterKeypair,
            anchor.getProvider().connection
          ),
          airdropSolIfNeeded(
            putMakerKeypair,
            anchor.getProvider().connection
          ),
          airdropSolIfNeeded(
            putMaker2Keypair,
            anchor.getProvider().connection
          ),
          airdropSolIfNeeded(
            putTakerKeypair,
            anchor.getProvider().connection
          ),
          airdropSolIfNeeded(
            protocolFeesKeypair,
            anchor.getProvider().connection
          ),
          airdropSolIfNeeded(
            callMakerKeypair,
            anchor.getProvider().connection
          ),
          airdropSolIfNeeded(
            callMaker2Keypair,
            anchor.getProvider().connection
          ),
          airdropSolIfNeeded(
            callTakerKeypair,
            anchor.getProvider().connection
          )
        ]
        await Promise.all(airdrops)
      } 
    );
  
    _testInitializeOracleAccount(anchor.getProvider().connection)  
  
    it("Is initialized!", async () => {
  
      // Add your test here.
      const tx = await program.methods.initialize().rpc();
      console.log("Your transaction signature", tx);
    });
    it("Creating a call option maker vault", async () => {
      const conn = anchor.getProvider().connection
      const wBTCMintAmountTaker = 0.02
      const callMakerwBTCATA = await createTokenAccount(conn, minterKeypair, wormholeBTCToken, callMakerKeypair.publicKey)
      await mintTokens(conn, minterKeypair, wormholeBTCToken, callMakerwBTCATA.address, minterKeypair, wBTCMintAmountTaker)
      console.log(`Minted ${wBTCMintAmountTaker} wBTC to test call maker, he will create a call option from here`)
      let currEpoch = Math.floor(Date.now()/1000)
      let oneWeek = currEpoch + (7*24*60*60)
  
      let strikeInDollars = 29000
      const mintInfoUSDC = await token.getMint(conn, usdcToken)
      const mintInfoWBTC = await token.getMint(conn, wormholeBTCToken)
      let lamportPrice = strikeInDollars * (10 ** mintInfoUSDC.decimals)
      console.log(`Lamport price for ${strikeInDollars} is ${lamportPrice}`)
      const vaultParams = new MakerCreateCallOptionParams(
        {
          maturity: new anchor.BN(oneWeek+300),
          strike: new anchor.BN(lamportPrice),
          //lotSize is in 10^lot_size
          lotSize: -3,
          maxMakers: 100,
          maxTakers: 100,
          numLotsToSell: new anchor.BN(10),
          premiumLimit: new anchor.BN(0)  
        })
        const callOptionVaultFactoryAddress = await getCallOptionVaultFactoryPdaAddress(program, wormholeBTCToken, usdcToken, vaultParams.maturity, vaultParams.strike)

        console.log('Derived address for CALL vault factory is: ' + callOptionVaultFactoryAddress.toString())
        const beforeBalance = await conn.getBalance(callMakerKeypair.publicKey)
        console.log("Initial callmaker SOL balance is", beforeBalance / anchor.web3.LAMPORTS_PER_SOL)    
        let tx = await program.methods.makerNextCallOptionVaultId(vaultParams).accounts({
          initializer: callMakerKeypair.publicKey,
          vaultFactoryInfo: callOptionVaultFactoryAddress,
          baseAssetMint: wormholeBTCToken,
          quoteAssetMint: mintInfoUSDC.address
        }).signers([callMakerKeypair]).rpc(confirmOptions)
    
        //inspired by example in https://github.com/coral-xyz/anchor/blob/master/tests/cpi-returns/tests/cpi-return.ts
        console.log("Transaction Signature -> ", tx)
        let t = await conn.getTransaction(tx, {
          maxSupportedTransactionVersion: 0,
          commitment: "confirmed",
        });
        const [key, , buffer] = getReturnLog(t)
        assert.equal(key, program.programId)
        const reader = new borsh.BinaryReader(buffer)
        const vaultNumber = reader.readU64()
        assert.equal(vaultNumber.toNumber(), 1)
    
        const {
          callOptionVaultAddress, 
          vaultBaseAssetTreasury, 
          vaultQuoteAssetTreasury
        } = await getCallOptionVaultDerivedPdaAddresses(program, callOptionVaultFactoryAddress, wormholeBTCToken, usdcToken, vaultNumber)
    
        var tx2 = await program.methods.makerCreateCallOptionVault(vaultParams, vaultNumber).accounts({
          initializer: callMakerKeypair.publicKey,
          vaultFactoryInfo: callOptionVaultFactoryAddress,
          vaultInfo: callOptionVaultAddress,
          vaultBaseAssetTreasury: vaultBaseAssetTreasury,
          vaultQuoteAssetTreasury: vaultQuoteAssetTreasury,
          baseAssetMint: wormholeBTCToken,
          quoteAssetMint: usdcToken,
          makerBaseAssetAccount: callMakerwBTCATA.address,
        }).signers([callMakerKeypair]).rpc()
    
        console.log("Transaction Signature -> ", tx2)
        const afterBalance = await anchor.getProvider().connection.getBalance(callMakerKeypair.publicKey)
        console.log("Final callmaker SOL balance is", afterBalance / anchor.web3.LAMPORTS_PER_SOL)
        const updatedATA = await token.getAccount(anchor.getProvider().connection, callMakerwBTCATA.address)
        const initialLamports = wBTCMintAmountTaker * (10**mintInfoWBTC.decimals)
        const transferLamports = (10**vaultParams.lotSize)*vaultParams.numLotsToSell.toNumber()*(10**mintInfoWBTC.decimals)
        assert.equal(initialLamports, transferLamports+Number(updatedATA.amount))
    
        const vaultFactories = await getAllMaybeNotMaturedCallFactories(program)
        assert.equal(vaultFactories[0].account.isInitialized, true)
        assert.equal(vaultFactories[0].account.matured, false)
        assert.equal(vaultFactories[0].account.maturity.toNumber(), vaultParams.maturity.toNumber())
        assert.equal(vaultFactories[0].account.baseAsset.toString(), wormholeBTCToken.toString())
        assert.equal(vaultFactories[0].account.quoteAsset.toString(), usdcToken.toString())
        assert.equal(vaultFactories[0].account.strike.toNumber(), vaultParams.strike.toNumber())
        const factoryKey = vaultFactories[0].publicKey
    
        const vaultsForFactory = await getVaultsForCallFactory(program, factoryKey)
        assert.equal(vaultsForFactory[0].account.maxMakers, vaultParams.maxMakers)
        assert.equal(vaultsForFactory[0].account.maxTakers, vaultParams.maxTakers)
    
        const userInfoInVault = await getUserMakerInfoAllCallVaults(program, callMakerKeypair.publicKey)
        assert.equal(userInfoInVault[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
    
        const makerInfos = await getAllCallMakerInfosForVault(program, vaultsForFactory[0].publicKey)
        assert.equal(makerInfos[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
    
        const makerInfoForVault = await getUserMakerInfoForCallVault(program, vaultsForFactory[0].publicKey, callMakerKeypair.publicKey)
        assert.equal(makerInfoForVault[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())        

        console.log('Now a second call maker will enter the same vault')
        const callMaker2wBTCCATA = await createTokenAccount(conn, minterKeypair, wormholeBTCToken, callMaker2Keypair.publicKey)
        const wbtcMint2Amount = 0.08
        await mintTokens(conn, minterKeypair, wormholeBTCToken, callMaker2wBTCCATA.address, minterKeypair, wbtcMint2Amount)
        console.log('Minted 0.08 wBTC to test call maker 2')
        const callMaker2ATA = await token.getOrCreateAssociatedTokenAccount(conn, minterKeypair, wormholeBTCToken, callMaker2Keypair.publicKey)
        const callOptionVaultFactoryAddress2 = await getCallOptionVaultFactoryPdaAddress(program, wormholeBTCToken, usdcToken, vaultParams.maturity, vaultParams.strike)
        const vaultInfo = (await getVaultsForCallFactory(program, callOptionVaultFactoryAddress2))[0]
        const vaultBaseAssetTreasury2 = await token.getAssociatedTokenAddress(wormholeBTCToken, vaultInfo.publicKey, true)
        const vaultQuoteAssetTreasury2 = await token.getAssociatedTokenAddress(usdcToken, vaultInfo.publicKey, true)

        try {
          let tx3 = await program.methods.makerEnterCallOptionVault(new anchor.BN(40), new anchor.BN(0)).accounts({
            initializer: callMaker2Keypair.publicKey,
            vaultFactoryInfo: callOptionVaultFactoryAddress2,
            vaultInfo: vaultInfo.publicKey,
            vaultBaseAssetTreasury: vaultBaseAssetTreasury2,
            baseAssetMint: wormholeBTCToken,
            quoteAssetMint: usdcToken,
            makerBaseAssetAccount: callMaker2ATA.address,
          }).signers([callMaker2Keypair]).rpc()
        } catch (e) {
          console.log(e)
          throw e
        }
        const makerInfos2 = await getAllCallMakerInfosForVault(program, vaultInfo.publicKey)
        // console.log(makerInfos2)
        assert.equal(makerInfos2.length, 2)
    
        let maker2InfoForVault = await getUserMakerInfoForCallVault(program, vaultInfo.publicKey, callMaker2Keypair.publicKey)
        const qty40Lots = maker2InfoForVault[0].account.baseAssetQty.toNumber()
        assert.isTrue(qty40Lots > 0)

        // testing maker adjust position
        let tx4 = await program.methods.makerAdjustPositionCallOptionVault(new anchor.BN(0), new anchor.BN(0)).accounts({        
          initializer: callMaker2Keypair.publicKey,
          vaultFactoryInfo: callOptionVaultFactoryAddress2,
          vaultInfo: vaultInfo.publicKey,
          vaultBaseAssetTreasury: vaultBaseAssetTreasury2,
          callOptionMakerInfo: maker2InfoForVault[0].publicKey,
          baseAssetMint: wormholeBTCToken,
          quoteAssetMint: usdcToken,
          makerBaseAssetAccount: callMaker2ATA.address,
        }).signers([callMaker2Keypair]).rpc()
        maker2InfoForVault = await getUserMakerInfoForCallVault(program, vaultInfo.publicKey, callMaker2Keypair.publicKey)
        assert.equal(maker2InfoForVault[0].account.baseAssetQty.toNumber(), 0)
    
        let tx5 = await program.methods.makerAdjustPositionCallOptionVault(new anchor.BN(40), new anchor.BN(0)).accounts({
          
          initializer: callMaker2Keypair.publicKey,
          vaultFactoryInfo: callOptionVaultFactoryAddress2,
          vaultInfo: vaultInfo.publicKey,
          vaultBaseAssetTreasury: vaultBaseAssetTreasury2,
          callOptionMakerInfo: maker2InfoForVault[0].publicKey,
          baseAssetMint: wormholeBTCToken,
          quoteAssetMint: usdcToken,
          makerBaseAssetAccount: callMaker2ATA.address,
        }).signers([callMaker2Keypair]).rpc()
        maker2InfoForVault = await getUserMakerInfoForCallVault(program, vaultInfo.publicKey, callMaker2Keypair.publicKey)
        assert.equal(maker2InfoForVault[0].account.baseAssetQty.toNumber(), qty40Lots)
  
        //Starting call taker simulation
        const oracleAddress = getOraclePubKey()

        const ticketAddress = await getUserTicketAccountAddressForCallVaultFactory(program, callOptionVaultFactoryAddress2, callTakerKeypair.publicKey)
    
        console.log("Call taker before paying oracle SOL balance is", await anchor.getProvider().connection.getBalance(callTakerKeypair.publicKey)/ anchor.web3.LAMPORTS_PER_SOL)
        console.log("Oracle SOL balance is", await anchor.getProvider().connection.getBalance(oracleAddress)/ anchor.web3.LAMPORTS_PER_SOL)
    
    
        let tx6 = await program.methods.genUpdateCallOptionFairPriceTicket().accounts({
          vaultFactoryInfo: callOptionVaultFactoryAddress2,
          initializer: callTakerKeypair.publicKey,
          oracleWallet: oracleAddress,
          callOptionFairPriceTicket: ticketAddress
        }).signers([callTakerKeypair]).rpc()
    
        console.log("Call taker after paying oracle SOL balance is", await anchor.getProvider().connection.getBalance(callTakerKeypair.publicKey)/ anchor.web3.LAMPORTS_PER_SOL)    
        console.log("Oracle SOL balance is", await anchor.getProvider().connection.getBalance(oracleAddress)/ anchor.web3.LAMPORTS_PER_SOL)
    
        let tx7 = await updateCallOptionFairPrice(program, callOptionVaultFactoryAddress2, callTakerKeypair.publicKey)
        console.log("Oracle SOL balance after updating fair price is", await anchor.getProvider().connection.getBalance(oracleAddress)/ anchor.web3.LAMPORTS_PER_SOL)
        console.log("Call taker after oracle using ticket SOL balance is", await anchor.getProvider().connection.getBalance(callTakerKeypair.publicKey)/ anchor.web3.LAMPORTS_PER_SOL)        
        let updatedVaultFactory = await program.account.callOptionVaultFactoryInfo.fetch(callOptionVaultFactoryAddress2)
        const fairPrice = updatedVaultFactory.lastFairPrice.toNumber()
        console.log('Updated call price is ', fairPrice)

        const slippageTolerance = 0.05      
        let sellers = await getCallSellersInVault(program, vaultInfo.publicKey, fairPrice, slippageTolerance)
        assert.equal(sellers.length, 2)

        const callTakerUSDCATA = await createTokenAccount(conn, minterKeypair, usdcToken, callTakerKeypair.publicKey)
      
        const usdcMintAmountTaker = 10000
        await mintTokens(conn, minterKeypair, usdcToken, callTakerUSDCATA.address, minterKeypair, usdcMintAmountTaker)
        console.log('Minted 10k usdc to test call taker, in order to pay call option premium and fund her option')
  
        
    });

    it("Creating a put option maker vault", async () => {
  
      const conn = anchor.getProvider().connection
      const putMakerUSDCATA = await createTokenAccount(conn, minterKeypair, usdcToken, putMakerKeypair.publicKey)
      
      const usdcMintAmount = 100000
      await mintTokens(conn, minterKeypair, usdcToken, putMakerUSDCATA.address, minterKeypair, usdcMintAmount)
      console.log('Minted 100k usdc to test put maker')
      const mintInfoUSDC = await token.getMint(conn, usdcToken)
      console.log(`USDC has ${mintInfoUSDC.decimals} decimals`)
      const updatedATA = await token.getOrCreateAssociatedTokenAccount(conn, minterKeypair, usdcToken, putMakerKeypair.publicKey)
      const balance = updatedATA.amount / BigInt(10.0 ** mintInfoUSDC.decimals)
      expect(balance).eq(BigInt(usdcMintAmount))
      const mintInfoWBTC = await token.getMint(conn, wormholeBTCToken)
      console.log(`WBTC has ${mintInfoWBTC.decimals} decimals`)
  
      let currEpoch = Math.floor(Date.now()/1000)
      let oneWeek = currEpoch + (7*24*60*60)
  
      let strikeInDollars = 26000
  
      let lamportPrice = strikeInDollars * (10 ** mintInfoUSDC.decimals)
      console.log(`Lamport price for ${strikeInDollars} is ${lamportPrice}`)
  
      const vaultParams = new MakerCreatePutOptionParams(
        {
          maturity: new anchor.BN(oneWeek+300),
          strike: new anchor.BN(lamportPrice),
          //lotSize is in 10^lot_size
          lotSize: -3,
          maxMakers: 100,
          maxTakers: 100,
          numLotsToSell: new anchor.BN(1000),
          premiumLimit: new anchor.BN(Math.floor(lamportPrice/100))  
        })
  
      const putOptionVaultFactoryAddress = await getPutOptionVaultFactoryPdaAddress(program, wormholeBTCToken, usdcToken, vaultParams.maturity, vaultParams.strike)
      
      console.log('Derived address for vault factory is: ' + putOptionVaultFactoryAddress.toString())
      const beforeBalance = await anchor.getProvider().connection.getBalance(putMakerKeypair.publicKey)
      console.log("Initial putmaker SOL balance is", beforeBalance / anchor.web3.LAMPORTS_PER_SOL)    
      let tx = await program.methods.makerNextPutOptionVaultId(vaultParams).accounts({
        initializer: putMakerKeypair.publicKey,
        vaultFactoryInfo: putOptionVaultFactoryAddress,
        baseAssetMint: mintInfoWBTC.address,
        quoteAssetMint: mintInfoUSDC.address
      }).signers([putMakerKeypair]).rpc(confirmOptions)
  
      //inspired by example in https://github.com/coral-xyz/anchor/blob/master/tests/cpi-returns/tests/cpi-return.ts
      console.log("Transaction Signature -> ", tx)
      let t = await anchor.getProvider().connection.getTransaction(tx, {
        maxSupportedTransactionVersion: 0,
        commitment: "confirmed",
      });
      const [key, , buffer] = getReturnLog(t)
      assert.equal(key, program.programId)
      const reader = new borsh.BinaryReader(buffer)
      const vaultNumber = reader.readU64()
      assert.equal(vaultNumber.toNumber(), 1)
  
      const {
        putOptionVaultAddress, 
        vaultBaseAssetTreasury, 
        vaultQuoteAssetTreasury
      } = await getPutOptionVaultDerivedPdaAddresses(program, putOptionVaultFactoryAddress, wormholeBTCToken, usdcToken, vaultNumber)
  
      //const userAVA = getMakerVaultAssociatedAccountAddress(program, putOptionVaultFactoryAddress, vaultNumber, putMakerKeypair.publicKey)
  
      var tx2 = await program.methods.makerCreatePutOptionVault(vaultParams, vaultNumber).accounts({
        initializer: putMakerKeypair.publicKey,
        vaultFactoryInfo: putOptionVaultFactoryAddress,
        vaultInfo: putOptionVaultAddress,
        vaultBaseAssetTreasury: vaultBaseAssetTreasury,
        vaultQuoteAssetTreasury: vaultQuoteAssetTreasury,
        baseAssetMint: wormholeBTCToken,
        quoteAssetMint: usdcToken,
        makerQuoteAssetAccount: updatedATA.address,
      }).signers([putMakerKeypair]).rpc()
  
      console.log("Transaction Signature -> ", tx2)
      const afterBalance = await anchor.getProvider().connection.getBalance(putMakerKeypair.publicKey)
      console.log("Final putmaker SOL balance is", afterBalance / anchor.web3.LAMPORTS_PER_SOL)
  
      const vaultFactories = await getAllMaybeNotMaturedPutFactories(program)
      assert.equal(vaultFactories[0].account.isInitialized, true)
      assert.equal(vaultFactories[0].account.matured, false)
      assert.equal(vaultFactories[0].account.maturity.toNumber(), vaultParams.maturity.toNumber())
      assert.equal(vaultFactories[0].account.baseAsset.toString(), wormholeBTCToken.toString())
      assert.equal(vaultFactories[0].account.quoteAsset.toString(), usdcToken.toString())
      assert.equal(vaultFactories[0].account.strike.toNumber(), vaultParams.strike.toNumber())
      const factoryKey = vaultFactories[0].publicKey
  
      const vaultsForFactory = await getVaultsForPutFactory(program, factoryKey)
      assert.equal(vaultsForFactory[0].account.maxMakers, vaultParams.maxMakers)
      assert.equal(vaultsForFactory[0].account.maxTakers, vaultParams.maxTakers)
  
      const userInfoInVault = await getUserMakerInfoAllPutVaults(program, putMakerKeypair.publicKey)
      assert.equal(userInfoInVault[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
  
      const makerInfos = await getAllPutMakerInfosForVault(program, vaultsForFactory[0].publicKey)
      assert.equal(makerInfos[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
  
      const makerInfoForVault = await getUserMakerInfoForPutVault(program, vaultsForFactory[0].publicKey, putMakerKeypair.publicKey)
      assert.equal(makerInfoForVault[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
  
      console.log("Second maker entering the same put option vault")
      const putMaker2USDCATA = await createTokenAccount(conn, minterKeypair, usdcToken, putMaker2Keypair.publicKey)
      const usdcMint2Amount = 50000
      await mintTokens(conn, minterKeypair, usdcToken, putMaker2USDCATA.address, minterKeypair, usdcMint2Amount)
      console.log('Minted 50k usdc to test put maker 2')
      const updatedATA2 = await token.getOrCreateAssociatedTokenAccount(conn, minterKeypair, usdcToken, putMaker2Keypair.publicKey)
      const balance2 = updatedATA2.amount / BigInt(10.0 ** mintInfoUSDC.decimals)
      expect(balance2).eq(BigInt(usdcMint2Amount))
      const putOptionVaultFactoryAddress2 = await getPutOptionVaultFactoryPdaAddress(program, wormholeBTCToken, usdcToken, vaultParams.maturity, vaultParams.strike)
      const vaultInfo = (await getVaultsForPutFactory(program, putOptionVaultFactoryAddress2))[0]
      const vaultBaseAssetTreasury2 = await token.getAssociatedTokenAddress(wormholeBTCToken, vaultInfo.publicKey, true)
      const vaultQuoteAssetTreasury2 = await token.getAssociatedTokenAddress(usdcToken, vaultInfo.publicKey, true)
  
      let tx3 = await program.methods.makerEnterPutOptionVault(new anchor.BN(500), new anchor.BN(0)).accounts({
        initializer: putMaker2Keypair.publicKey,
        vaultFactoryInfo: putOptionVaultFactoryAddress2,
        vaultInfo: vaultInfo.publicKey,
        vaultQuoteAssetTreasury: vaultQuoteAssetTreasury2,
        baseAssetMint: wormholeBTCToken,
        quoteAssetMint: usdcToken,
        makerQuoteAssetAccount: updatedATA2.address,
      }).signers([putMaker2Keypair]).rpc()
      const makerInfos2 = await getAllPutMakerInfosForVault(program, vaultInfo.publicKey)
      // console.log(makerInfos2)
      assert.equal(makerInfos2.length, 2)
  
      let maker2InfoForVault = await getUserMakerInfoForPutVault(program, vaultInfo.publicKey, putMaker2Keypair.publicKey)
      const qty500Lots = maker2InfoForVault[0].account.quoteAssetQty.toNumber()
      assert.isTrue(qty500Lots > 0)
  
      let tx4 = await program.methods.makerAdjustPositionPutOptionVault(new anchor.BN(0), new anchor.BN(0)).accounts({
        
        initializer: putMaker2Keypair.publicKey,
        vaultFactoryInfo: putOptionVaultFactoryAddress2,
        vaultInfo: vaultInfo.publicKey,
        vaultQuoteAssetTreasury: vaultQuoteAssetTreasury2,
        putOptionMakerInfo: maker2InfoForVault[0].publicKey,
        baseAssetMint: wormholeBTCToken,
        quoteAssetMint: usdcToken,
        makerQuoteAssetAccount: updatedATA2.address,
      }).signers([putMaker2Keypair]).rpc()
      maker2InfoForVault = await getUserMakerInfoForPutVault(program, vaultInfo.publicKey, putMaker2Keypair.publicKey)
      assert.equal(maker2InfoForVault[0].account.quoteAssetQty.toNumber(), 0)
  
      let tx5 = await program.methods.makerAdjustPositionPutOptionVault(new anchor.BN(500), new anchor.BN(0)).accounts({
        
        initializer: putMaker2Keypair.publicKey,
        vaultFactoryInfo: putOptionVaultFactoryAddress2,
        vaultInfo: vaultInfo.publicKey,
        vaultQuoteAssetTreasury: vaultQuoteAssetTreasury2,
        putOptionMakerInfo: maker2InfoForVault[0].publicKey,
        baseAssetMint: wormholeBTCToken,
        quoteAssetMint: usdcToken,
        makerQuoteAssetAccount: updatedATA2.address,
      }).signers([putMaker2Keypair]).rpc()
      maker2InfoForVault = await getUserMakerInfoForPutVault(program, vaultInfo.publicKey, putMaker2Keypair.publicKey)
      assert.equal(maker2InfoForVault[0].account.quoteAssetQty.toNumber(), qty500Lots)
  
      let fairPrice = Math.floor(lamportPrice/100)
      const slippageTolerance = 0.05
  
      fairPrice -= 100000000
      let sellers = await getPutSellersInVault(program, vaultInfo.publicKey, fairPrice, slippageTolerance)
      // fairPrice below premium limit of 1st maker
      assert.equal(sellers.length, 1)
  
      fairPrice += 100000000
      sellers = await getPutSellersInVault(program, vaultInfo.publicKey, fairPrice, slippageTolerance)
      // now fairPrice is in the range of both sellers
      assert.equal(sellers.length, 2)
  
      //Starting taker simulation
      const connection = new anchor.web3.Connection(anchor.getProvider().connection.rpcEndpoint, {commitment: 'confirmed'})
      const currentSlot = await connection.getSlot();
      console.log('currentSlot:', currentSlot);    
  
      const slots = await connection.getBlocks(Math.max(currentSlot - 200, 0));
      const oracleAddress = getOraclePubKey()

      const ticketAddress = await getUserTicketAccountAddressForPutVaultFactory(program, putOptionVaultFactoryAddress2, putTakerKeypair.publicKey)
  
      console.log("Put taker before paying oracle SOL balance is", await anchor.getProvider().connection.getBalance(putTakerKeypair.publicKey)/ anchor.web3.LAMPORTS_PER_SOL)
      console.log("Oracle SOL balance is", await anchor.getProvider().connection.getBalance(oracleAddress)/ anchor.web3.LAMPORTS_PER_SOL)
  
  
      let tx6 = await program.methods.genUpdatePutOptionFairPriceTicket().accounts({
        vaultFactoryInfo: putOptionVaultFactoryAddress2,
        initializer: putTakerKeypair.publicKey,
        oracleWallet: oracleAddress,
        putOptionFairPriceTicket: ticketAddress
      }).signers([putTakerKeypair]).rpc()
  
      console.log("Put taker after paying oracle SOL balance is", await anchor.getProvider().connection.getBalance(putTakerKeypair.publicKey)/ anchor.web3.LAMPORTS_PER_SOL)    
      console.log("Oracle SOL balance is", await anchor.getProvider().connection.getBalance(oracleAddress)/ anchor.web3.LAMPORTS_PER_SOL)
  
      let tx7 = await updatePutOptionFairPrice(program, putOptionVaultFactoryAddress2, putTakerKeypair.publicKey)
      console.log("Oracle SOL balance after updating fair price is", await anchor.getProvider().connection.getBalance(oracleAddress)/ anchor.web3.LAMPORTS_PER_SOL)
      console.log("Put taker after oracle using ticket SOL balance is", await anchor.getProvider().connection.getBalance(putTakerKeypair.publicKey)/ anchor.web3.LAMPORTS_PER_SOL)        
      let updatedVaultFactory = await program.account.putOptionVaultFactoryInfo.fetch(putOptionVaultFactoryAddress2)
      console.log(updatedVaultFactory.lastFairPrice.toNumber())
      sellers = await getPutSellersInVault(program, vaultInfo.publicKey, updatedVaultFactory.lastFairPrice.toNumber(), slippageTolerance)
      //console.log('sellers in vault')
      //console.log(sellers)
  
      const putTakerUSDCATA = await createTokenAccount(conn, minterKeypair, usdcToken, putTakerKeypair.publicKey)
      
      const usdcMintAmountTaker = 10000
      await mintTokens(conn, minterKeypair, usdcToken, putTakerUSDCATA.address, minterKeypair, usdcMintAmountTaker)
      console.log('Minted 10k usdc to test put taker, in order to pay put option premium')
  
      const putTakerwBTCATA = await createTokenAccount(conn, minterKeypair, wormholeBTCToken, putTakerKeypair.publicKey)
      const wBTCMintAmountTaker = 10
      await mintTokens(conn, minterKeypair, wormholeBTCToken, putTakerwBTCATA.address, minterKeypair, wBTCMintAmountTaker)
      console.log('Minted 10 wBTC to test put taker, he will eventually fund his put option from here')
  
      //Lets suppose put taker slippage tolerance is 5%
      const myMaxPrice = Math.floor(updatedVaultFactory.lastFairPrice.toNumber()*1.05)
      // notice that we defined above that the lot size of our vault is 0.001 bitcoin
      // first maker in the vault is selling 1000 lots
      // second maker in the vault is selling 500 lots
      // we are buying the right to sell 600 lots, that is 0.6 bitcoin
      // however the taker will initially fund the option with only 0.1 bitcoin
      const btcLamports = Math.round(0.1*(10 ** mintInfoWBTC.decimals))
      const takerLots = 600
  
      const protocolFeesUSDCATA = await createTokenAccount(conn, minterKeypair, usdcToken, protocolFeesKeypair.publicKey)
  
      const tokenAccountTestFetch = await token.getAccount(conn, protocolFeesUSDCATA.address)
      assert.equal(tokenAccountTestFetch.address, protocolFeesUSDCATA.address)
  
      let sellersAndATAS = await getPutMakerATAs(program, sellers, usdcToken)
      console.log("SELLERS AND ATAS")
      console.log(sellersAndATAS)
      const quoteAssetByLot = (10**vaultInfo.account.lotSize)*updatedVaultFactory.strike.toNumber()
      const lotsInQuoteAsset = takerLots*quoteAssetByLot
      console.log(`${takerLots} lots of ${10**vaultInfo.account.lotSize} at strike price ${updatedVaultFactory.strike.toNumber()} mean ${lotsInQuoteAsset} in USDC lamports`)
      
      const remainingAccounts = await getPutSellersAsRemainingAccounts(takerLots, program, sellers)
      
      try {
        let tx8 = await program.methods.takerBuyLotsPutOptionVault(
          new anchor.BN(myMaxPrice), 
          new anchor.BN(takerLots), 
          new anchor.BN(btcLamports)).accounts({
            baseAssetMint: wormholeBTCToken,
            quoteAssetMint: usdcToken,
            initializer: putTakerKeypair.publicKey,
            protocolQuoteAssetTreasury: protocolFeesUSDCATA.address,
            frontendQuoteAssetTreasury: protocolFeesUSDCATA.address, //also sending frontend share to protocol in this test
            takerBaseAssetAccount: putTakerwBTCATA.address,
            takerQuoteAssetAccount: putTakerUSDCATA.address,
            vaultFactoryInfo: putOptionVaultFactoryAddress2,
            vaultInfo: vaultInfo.publicKey,
            vaultBaseAssetTreasury: vaultBaseAssetTreasury2,
          }).remainingAccounts(
            remainingAccounts
          ).signers([putTakerKeypair]).rpc()
    
          console.log("ALL DONE")
      } catch(e) {
        console.log(e)
      }
  
    });  

  }

  
    // Scratch area    
    //console.log(slots)

    //Inspired by https://github.com/solana-developers/web3-examples/tree/main/address-lookup-tables
    //See also:     https://www.youtube.com/watch?v=8k68cMeLX2U
    /*
    const [lookupTableInst, lookupTableAddress] = anchor.web3.AddressLookupTableProgram.createLookupTable({
      authority: putTakerKeypair.publicKey,
      payer: putTakerKeypair.publicKey,
      recentSlot: slots[0]
    })
    const txId = await sendTransactionV0(connection, [lookupTableInst], putTakerKeypair)
    console.log('Waiting for Address Lookup Table creation confirmation')
    await confirm(txId);    
    const sellerAddresses = sellers.map(a => a.publicKey)
    const ix = anchor.web3.AddressLookupTableProgram.extendLookupTable({
			addresses: sellerAddresses,
			authority: putTakerKeypair.publicKey,
			lookupTable: lookupTableAddress,
			payer: putTakerKeypair.publicKey,
		});
    const tx2Id = await sendTransactionV0(connection, [ix], putTakerKeypair)
    console.log('Waiting for Address Lookup Table extension confirmation')
    await confirm(tx2Id);

    async function confirm(tx: string) {
      const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash();
      await connection.confirmTransaction({
        blockhash,
        lastValidBlockHeight,
        signature: tx
      }, 'singleGossip');
    }

    const lookupTableAccount = await connection
		.getAddressLookupTable(lookupTableAddress)
		.then((res) => res.value);
    assert.equal(lookupTableAccount.state.addresses.length, 2)

    anchor.web3.AddressLookupTableProgram.closeLookupTable({
      lookupTable: lookupTableAddress,
      authority: putTakerKeypair.publicKey,
      recipient: putTakerKeypair.publicKey,
    })
    */
    

});
