import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SnakeMinterDevnet } from "../target/types/snake_minter_devnet";
import { assert, expect } from "chai";
import * as dotenv from "dotenv";
import { createTokenAccount } from "./anchor-solhedge";
import * as token from "@solana/spl-token"
import { isLocalnet, keyPairFromSecret } from "./util";

dotenv.config()

const DEVNET_DEVEL_KEY = process.env.PRIVATE_KEY;

// Mint address for SnakeDollar, get it from snake-tokens/snD.json ("mint")
export const snakeDollarMintAddr = new anchor.web3.PublicKey("BJvndCYS1eMf1bg6vyJCjZiUEFcnZ5DeZKJiyZCjwN6K")

// Mint address for SnakeBTC, get it from snake-tokens/snBTC.json ("mint")
export const snakeBTCMintAddr = new anchor.web3.PublicKey("6p728Y98qrSrvjRQmmvRLqa3JJ4P9RyLwbJ42DHxG7tP")

export const mintSnakeDollarTo = async (
  program: anchor.Program<SnakeMinterDevnet>,
  user: anchor.web3.Keypair
): Promise<string> => {
  
  const snakeDollarUserATA = await createTokenAccount(program.provider.connection, user, snakeDollarMintAddr, user.publicKey);
  const [mintAuthAddress, _mintAuthBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from(anchor.utils.bytes.utf8.encode("mint")),
    ],
    program.programId
  )
  console.log(`Minting 500 SnakeDollars to ${user.publicKey}`)
  const tx = program.methods.mintSnd().accounts({
    initializer: user.publicKey,
    snakeDollarMint: snakeDollarMintAddr,
    userSndAta: snakeDollarUserATA.address,
    snakeMintAuth: mintAuthAddress
  }).signers([user]).rpc()
  return tx
}

export const mintSnakeBTCTo = async (
  program: anchor.Program<SnakeMinterDevnet>,
  user: anchor.web3.Keypair
): Promise<string> => {
  
  const snakeBTCUserATA = await createTokenAccount(program.provider.connection, user, snakeBTCMintAddr, user.publicKey);
  const [mintAuthAddress, _mintAuthBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from(anchor.utils.bytes.utf8.encode("mint")),
    ],
    program.programId
  )
  console.log(`Minting 0.2 SnakeBTC to ${user.publicKey}`)
  const tx = await program.methods.mintSnbtc().accounts({
    initializer: user.publicKey,
    snakeBtcMint: snakeBTCMintAddr,
    userSnbtcAta: snakeBTCUserATA.address,
    snakeMintAuth: mintAuthAddress
  }).signers([user]).rpc()    
  return tx
}

xdescribe("snake-minter-devnet", () => {
    anchor.setProvider(anchor.AnchorProvider.env());
    if (!isLocalnet(anchor.getProvider().connection)) {
        console.log('For now, this means we\'re at devnet!');

        const program = anchor.workspace.SnakeMinterDevnet as Program<SnakeMinterDevnet>;

        const payerKeypair = keyPairFromSecret(JSON.parse(DEVNET_DEVEL_KEY))



        it(`Minting 500 SnakeDollars to ${payerKeypair.publicKey}`, async () => {
            const tx = await mintSnakeDollarTo(program, payerKeypair)
            console.log("Mint snake dollars transaction signature", tx);
        });

          it(`Minting 0.2 SnakeBTC to ${payerKeypair.publicKey}`, async () => {
            const tx = await mintSnakeBTCTo(program, payerKeypair)
            console.log("Mint snake BTC transaction signature", tx);
          });

    }
});
