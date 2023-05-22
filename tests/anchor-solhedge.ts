import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { AnchorSolhedge } from "../target/types/anchor_solhedge";
import * as token from "@solana/spl-token"
import { expect } from "chai";

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

    const conn = anchor.getProvider().connection
    const putMakerUSDCATA = await createTokenAccount(conn, putMakerKeypair, usdcToken, putMakerKeypair.publicKey)
    
    const usdcMintAmount = 100000
    await mintTokens(conn, putMakerKeypair, usdcToken, putMakerUSDCATA.address, minterKeypair, usdcMintAmount)
    console.log('Minted 100k usdc to test put maker')
    const mintInfo = await token.getMint(conn, usdcToken)
    const updatedATA = await token.getOrCreateAssociatedTokenAccount(conn, putMakerKeypair, usdcToken, putMakerKeypair.publicKey)
    const balance = updatedATA.amount / BigInt(10.0 ** mintInfo.decimals)
    expect(balance).eq(BigInt(usdcMintAmount))

    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
