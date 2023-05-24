import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { AnchorSolhedge } from "../target/types/anchor_solhedge";
import * as token from "@solana/spl-token"
import { assert, expect } from "chai";
import { 
  MakerCreatePutOptionParams, 
  getVaultFactoryPdaAddress, 
  getVaultDerivedPdaAddresses, 
  getUserVaultAssociatedAccountAddress,
  getAllMaybeNotMaturedFactories,
  getVaultsForPutFactory,
  getUserMakerInfoAllVaults
} from "./accounts";
import * as borsh from "borsh";


const TEST_PUT_MAKER_KEY = [7,202,200,249,141,19,80,240,20,148,116,158,237,253,235,157,26,157,95,58,241,232,6,221,233,94,248,189,255,95,87,169,170,77,151,133,53,15,237,214,51,0,2,67,60,75,202,138,200,234,155,157,153,141,162,233,83,179,126,125,248,211,212,51]

// The corresponding pubkey of this key is what we should put in pyutil/replaceMint.py to generate the mocks USDC and WBTC
const TEST_MOCK_MINTER_KEY = [109,3,86,101,96,42,254,204,98,232,34,172,105,37,112,24,223,194,66,133,2,105,54,228,54,97,90,111,253,35,245,73,93,83,136,36,51,237,111,8,250,149,126,98,135,211,138,191,207,116,66,179,204,231,147,190,217,190,220,93,181,102,164,238]

function keyPairFromSecret(secret: number[]): anchor.web3.Keypair {
  const secretKey = Uint8Array.from(secret)
  const keypair = anchor.web3.Keypair.fromSecretKey(secretKey)
  //console.log(keypair.publicKey.toString())
  return keypair
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

async function createTokenAccount(
  connection: anchor.web3.Connection,
  payer: anchor.web3.Keypair,
  mint: anchor.web3.PublicKey,
  owner: anchor.web3.PublicKey
) {
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


describe("anchor-solhedge", () => {
  //console.log(anchor.AnchorProvider.env())
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.AnchorSolhedge as Program<AnchorSolhedge>;

  const minterKeypair = keyPairFromSecret(TEST_MOCK_MINTER_KEY)
  const putMakerKeypair = keyPairFromSecret(TEST_PUT_MAKER_KEY)

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
        )
      ]
      await Promise.all(airdrops)
    } 
  );

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
    const updatedATA = await token.getOrCreateAssociatedTokenAccount(conn, minterKeypair, usdcToken, putMakerKeypair.publicKey)
    const balance = updatedATA.amount / BigInt(10.0 ** mintInfoUSDC.decimals)
    expect(balance).eq(BigInt(usdcMintAmount))
    const mintInfoWBTC = await token.getMint(conn, wormholeBTCToken)

    let currEpoch = Math.floor(Date.now()/1000)
    let tomorrow = currEpoch + (24*60*60)

    let strikeInDollars = 29000
    console.log(`USDC decimals: ${mintInfoUSDC.decimals}, WBTC decimals: ${mintInfoWBTC.decimals}`)

    let lamportPrice = strikeInDollars * (10 ** (mintInfoUSDC.decimals - mintInfoWBTC.decimals))
    console.log(`Lamport price for ${strikeInDollars} is ${lamportPrice}`)

    const vaultParams = new MakerCreatePutOptionParams(
      {
        maturity: new anchor.BN(tomorrow),
        strike: new anchor.BN(lamportPrice),
        lotSize: new anchor.BN(10000),
        maxMakers: 100,
        maxTakers: 100,
        minTickerIncrement: 0.01,
        numLotsToSell: new anchor.BN(1000),
        premiumLimit: new anchor.BN(lamportPrice/100)  
      })

    const putOptionVaultFactoryAddress = await getVaultFactoryPdaAddress(program, wormholeBTCToken, usdcToken, vaultParams)
    
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

    const userAVA = getUserVaultAssociatedAccountAddress(program, putOptionVaultFactoryAddress, vaultNumber, putMakerKeypair.publicKey)
    let tx2 = await program.methods.makerCreatePutOptionVault(vaultParams, vaultNumber).accounts({
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
    console.log(userInfoInVault)
  });
});
