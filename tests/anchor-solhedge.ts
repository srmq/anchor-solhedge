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
  getVaultFactoryPdaAddress, 
  getVaultDerivedPdaAddresses, 
  getMakerVaultAssociatedAccountAddress,
  getAllMaybeNotMaturedFactories,
  getVaultsForPutFactory,
  getUserMakerInfoAllVaults,
  getAllMakerInfosForVault,
  getUserMakerInfoForVault,
  getSellersInVault,
  getUserTicketAccountAddressForVaultFactory,
  getMakerATAs,
  getMakerNextPutOptionVaultIdFromTx
} from "./accounts";
import * as borsh from "borsh";
import { getOraclePubKey, _testInitializeOracleAccount, updatePutOptionFairPrice, lastKnownPrice } from "./oracle";
import { snakeBTCMintAddr, snakeDollarMintAddr, mintSnakeDollarTo } from "./snake-minter-devnet";
import { oracleAddr } from "./oracle";

dotenv.config()

const TEST_PUT_MAKER_KEY = [7,202,200,249,141,19,80,240,20,148,116,158,237,253,235,157,26,157,95,58,241,232,6,221,233,94,248,189,255,95,87,169,170,77,151,133,53,15,237,214,51,0,2,67,60,75,202,138,200,234,155,157,153,141,162,233,83,179,126,125,248,211,212,51]
const TEST_PUT_MAKER2_KEY = [58,214,126,90,15,29,80,114,170,70,234,58,244,144,25,23,110,1,6,19,176,12,232,59,55,64,56,53,60,187,246,157,140,117,187,255,239,135,134,192,94,254,53,137,53,27,99,244,218,86,207,59,22,189,242,164,155,104,68,250,161,179,108,4]

const TEST_PUT_TAKER_KEY = [198,219,91,244,252,118,0,25,83,232,178,61,51,196,168,151,77,1,142,9,164,80,29,63,76,216,213,85,99,185,71,113,36,61,101,115,203,92,102,70,200,37,98,228,234,240,155,7,144,0,244,71,236,104,22,131,143,216,47,244,151,205,246,245]

// The corresponding pubkey of this key is what we should put in pyutil/replaceMint.py to generate the mocks USDC and WBTC
const TEST_MOCK_MINTER_KEY = [109,3,86,101,96,42,254,204,98,232,34,172,105,37,112,24,223,194,66,133,2,105,54,228,54,97,90,111,253,35,245,73,93,83,136,36,51,237,111,8,250,149,126,98,135,211,138,191,207,116,66,179,204,231,147,190,217,190,220,93,181,102,164,238]

// This is the where protocol fees will go, its pubkey is in lib.rs. MUST CHANGE IN REAL DEPLOYMENT
const TEST_PROTOCOL_FEES_KEY = [170,187,172,146,241,33,174,135,129,205,0,108,30,54,58,190,112,43,95,133,59,63,136,89,167,183,88,187,65,45,66,214,212,13,191,146,112,52,37,80,118,225,123,85,122,18,26,51,145,227,30,224,105,163,126,21,155,210,207,191,239,81,83,244]

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
  
  console.log(
      `Token Account: ${tokenAccount.address}`
  )

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

  const devnetPayerKeypair = keyPairFromSecret(DEVNET_DEVEL_KEY)
  const putMaker1Keypair = keyPairFromSecret(DEVNET_PUTMAKER1_KEY)
  const putMaker2Keypair = keyPairFromSecret(DEVNET_PUTMAKER2_KEY)
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
          fundPeerIfNeeded(devnetPayerKeypair, protocolFeesAddr, anchor.getProvider().connection)
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


    xit(`Now ${putMaker1Keypair.publicKey} is creating a Vault Factory and a Vault inside it as a PutMaker`, async () => {
      const btcPrice = await lastKnownPrice("3NZ9JMVBmGAqocybic2c7LQCJScmgsAZ6vQqTDzcqmJh") //wBTC
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
      const putOptionVaultFactoryAddress = await getVaultFactoryPdaAddress(program, snakeBTCMintAddr, snakeDollarMintAddr, vaultParams.maturity, vaultParams.strike)

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
      } = await getVaultDerivedPdaAddresses(program, putOptionVaultFactoryAddress, snakeBTCMintAddr, snakeDollarMintAddr, vaultNumber)

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

      const vaultFactories = await getAllMaybeNotMaturedFactories(program)
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
  
      const userInfoInVault = await getUserMakerInfoAllVaults(program, putMaker1Keypair.publicKey)
      assert.equal(userInfoInVault[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
  
      const makerInfos = await getAllMakerInfosForVault(program, vaultsForFactory[0].publicKey)
      assert.equal(makerInfos[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
  
      const makerInfoForVault = await getUserMakerInfoForVault(program, vaultsForFactory[0].publicKey, putMaker1Keypair.publicKey)
      assert.equal(makerInfoForVault[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
    });

    it(`Now a second Putmaker, ${putMaker2Keypair.publicKey} will try to enter existing PutOptionVaults`, async () => {
      const vaultFactories = await getAllMaybeNotMaturedFactories(program)
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
            if ((await getUserMakerInfoForVault(program, vault.publicKey, putMaker2Keypair.publicKey)).length > 0) {
              continue
            }
            const minEntry = Math.ceil((10**vault.account.lotSize)*vaultFactory.account.strike.toNumber())
            const myBalance = await getTokenBalance(program.provider.connection, putMaker2Keypair, snakeDollarMintAddr, putMaker2Keypair.publicKey)
            const {
              putOptionVaultAddress, 
              vaultBaseAssetTreasury, 
              vaultQuoteAssetTreasury
            } = await getVaultDerivedPdaAddresses(program, vaultFactory.publicKey, vaultFactory.account.baseAsset, vaultFactory.account.quoteAsset, vault.account.ord)
      
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
              let maker2InfoForVault = await getUserMakerInfoForVault(program, vault.publicKey, putMaker2Keypair.publicKey)
              const quoteAssetQty = maker2InfoForVault[0].account.quoteAssetQty.toNumber()
              assert.isTrue(quoteAssetQty > 0)
        
            }
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
  
    const putTakerKeypair = keyPairFromSecret(TEST_PUT_TAKER_KEY)
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
  
      const putOptionVaultFactoryAddress = await getVaultFactoryPdaAddress(program, wormholeBTCToken, usdcToken, vaultParams.maturity, vaultParams.strike)
      
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
      } = await getVaultDerivedPdaAddresses(program, putOptionVaultFactoryAddress, wormholeBTCToken, usdcToken, vaultNumber)
  
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
  
      const vaultFactories = await getAllMaybeNotMaturedFactories(program)
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
  
      const userInfoInVault = await getUserMakerInfoAllVaults(program, putMakerKeypair.publicKey)
      assert.equal(userInfoInVault[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
  
      const makerInfos = await getAllMakerInfosForVault(program, vaultsForFactory[0].publicKey)
      assert.equal(makerInfos[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
  
      const makerInfoForVault = await getUserMakerInfoForVault(program, vaultsForFactory[0].publicKey, putMakerKeypair.publicKey)
      assert.equal(makerInfoForVault[0].account.premiumLimit.toNumber(), vaultParams.premiumLimit.toNumber())
  
      console.log("Second maker entering the same put option vault")
      const putMaker2USDCATA = await createTokenAccount(conn, minterKeypair, usdcToken, putMaker2Keypair.publicKey)
      const usdcMint2Amount = 50000
      await mintTokens(conn, minterKeypair, usdcToken, putMaker2USDCATA.address, minterKeypair, usdcMint2Amount)
      console.log('Minted 50k usdc to test put maker 2')
      const updatedATA2 = await token.getOrCreateAssociatedTokenAccount(conn, minterKeypair, usdcToken, putMaker2Keypair.publicKey)
      const balance2 = updatedATA2.amount / BigInt(10.0 ** mintInfoUSDC.decimals)
      expect(balance2).eq(BigInt(usdcMint2Amount))
      const putOptionVaultFactoryAddress2 = await getVaultFactoryPdaAddress(program, wormholeBTCToken, usdcToken, vaultParams.maturity, vaultParams.strike)
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
      const makerInfos2 = await getAllMakerInfosForVault(program, vaultInfo.publicKey)
      // console.log(makerInfos2)
      assert.equal(makerInfos2.length, 2)
  
      let maker2InfoForVault = await getUserMakerInfoForVault(program, vaultInfo.publicKey, putMaker2Keypair.publicKey)
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
      maker2InfoForVault = await getUserMakerInfoForVault(program, vaultInfo.publicKey, putMaker2Keypair.publicKey)
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
      maker2InfoForVault = await getUserMakerInfoForVault(program, vaultInfo.publicKey, putMaker2Keypair.publicKey)
      assert.equal(maker2InfoForVault[0].account.quoteAssetQty.toNumber(), qty500Lots)
  
      let fairPrice = Math.floor(lamportPrice/100)
      const slippageTolerance = 0.05
  
      fairPrice -= 100000000
      let sellers = await getSellersInVault(program, vaultInfo.publicKey, fairPrice, slippageTolerance)
      // fairPrice below premium limit of 1st maker
      assert.equal(sellers.length, 1)
  
      fairPrice += 100000000
      sellers = await getSellersInVault(program, vaultInfo.publicKey, fairPrice, slippageTolerance)
      // now fairPrice is in the range of both sellers
      assert.equal(sellers.length, 2)
  
      //Starting taker simulation
      const connection = new anchor.web3.Connection(anchor.getProvider().connection.rpcEndpoint, {commitment: 'confirmed'})
      const currentSlot = await connection.getSlot();
      console.log('currentSlot:', currentSlot);    
  
      const slots = await connection.getBlocks(Math.max(currentSlot - 200, 0));
      const oracleAddress = getOraclePubKey()

      const ticketAddress = await getUserTicketAccountAddressForVaultFactory(program, putOptionVaultFactoryAddress2, putTakerKeypair.publicKey)
  
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
      sellers = await getSellersInVault(program, vaultInfo.publicKey, updatedVaultFactory.lastFairPrice.toNumber(), slippageTolerance)
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
  
      let sellersAndATAS = await getMakerATAs(program, sellers, usdcToken)
      console.log("SELLERS AND ATAS")
      console.log(sellersAndATAS)
      const quoteAssetByLot = (10**vaultInfo.account.lotSize)*updatedVaultFactory.strike.toNumber()
      const lotsInQuoteAsset = takerLots*quoteAssetByLot
      console.log(`${takerLots} lots of ${10**vaultInfo.account.lotSize} at strike price ${updatedVaultFactory.strike.toNumber()} mean ${lotsInQuoteAsset} in USDC lamports`)
      
      // will we get the first 4, and the 5st may be one later if the fourth does not complete
      // enough demand
      var i = 0
      let remainingAccounts = []
      for (const [putOptionMakerInfo, makerATA] of sellersAndATAS) {
        let potentialLots = 0
        if (i < 4) {
          const remAccountInfo = {
            pubkey: putOptionMakerInfo.publicKey,
            isWritable: true,
            isSigner: false
          }
          const remAccountATA = {
            pubkey: makerATA.address,
            isWritable: true,
            isSigner: false
          }
          remainingAccounts.push(remAccountInfo);
          remainingAccounts.push(remAccountATA);
          const quoteAssetAvailable = putOptionMakerInfo.account.quoteAssetQty.toNumber() - putOptionMakerInfo.account.volumeSold.toNumber()
          const userPotentialLots = Math.floor(quoteAssetAvailable/quoteAssetByLot)
          potentialLots += userPotentialLots
          console.log(`User ${i} has at most ${userPotentialLots} lots to sell`)
        } else if (remainingAccounts.length >= 5) {
          break;
        } else if (i < sellersAndATAS.length-1){
          const quoteAssetAvailable = putOptionMakerInfo.account.quoteAssetQty.toNumber() - putOptionMakerInfo.account.volumeSold.toNumber()
          const userPotentialLots = Math.floor(quoteAssetAvailable/quoteAssetByLot)
          if (potentialLots + userPotentialLots >= takerLots) {
            const remAccountInfo = {
              pubkey: putOptionMakerInfo.publicKey,
              isWritable: true,
              isSigner: false
            }
            const remAccountATA = {
              pubkey: makerATA.address,
              isWritable: true,
              isSigner: false
            }
            remainingAccounts.push(remAccountInfo);
            remainingAccounts.push(remAccountATA);
            potentialLots += userPotentialLots;
            break; 
          }
        } else {
          // last chance, this last one or the 5th
          let quoteAssetAvailable = putOptionMakerInfo.account.quoteAssetQty.toNumber() - putOptionMakerInfo.account.volumeSold.toNumber()
          let userPotentialLots = Math.floor(quoteAssetAvailable/quoteAssetByLot)
          let makerPubkey = putOptionMakerInfo.publicKey
          let ataPubkey = makerATA.address
          if (potentialLots + userPotentialLots < takerLots) {
            makerPubkey = sellersAndATAS[4][0].publicKey
            ataPubkey = sellersAndATAS[4][1].address
            quoteAssetAvailable = sellersAndATAS[4][0].account.quoteAssetQty.toNumber() - sellersAndATAS[4][0].account.volumeSold.toNumber()
            userPotentialLots = Math.floor(quoteAssetAvailable/quoteAssetByLot)
          }
          const remAccountInfo = {
            pubkey: makerPubkey,
            isWritable: true,
            isSigner: false
          }
          const remAccountATA = {
            pubkey: ataPubkey,
            isWritable: true,
            isSigner: false
          }
          remainingAccounts.push(remAccountInfo);
          remainingAccounts.push(remAccountATA);
          potentialLots += userPotentialLots;
          break;
        }
        i++;
      }
  
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
