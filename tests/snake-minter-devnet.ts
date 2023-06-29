import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SnakeMinterDevnet } from "../target/types/snake_minter_devnet";
import { assert, expect } from "chai";
import * as dotenv from "dotenv";
import { isLocalnet, keyPairFromSecret, createTokenAccount } from "./anchor-solhedge";
import * as token from "@solana/spl-token"

dotenv.config()

const PAYER_PRIVATE_KEY = process.env.PRIVATE_KEY;

// Mint address for SnakeDollar, get it from snake-tokens/snD.json ("mint")
const snakeDollarMintAddr = new anchor.web3.PublicKey("BJvndCYS1eMf1bg6vyJCjZiUEFcnZ5DeZKJiyZCjwN6K")

// Mint address for SnakeBTC, get it from snake-tokens/snBTC.json ("mint")
const snakeBTCMintAddr = new anchor.web3.PublicKey("6p728Y98qrSrvjRQmmvRLqa3JJ4P9RyLwbJ42DHxG7tP")

describe("snake-minter-testnet", () => {
    anchor.setProvider(anchor.AnchorProvider.env());
    if (!isLocalnet(anchor.getProvider().connection)) {
        console.log('For now, this means we\'re at devnet!');

        const program = anchor.workspace.SnakeMinterDevnet as Program<SnakeMinterDevnet>;

        const payerKeypair = keyPairFromSecret(JSON.parse(PAYER_PRIVATE_KEY))



        it(`Minting 500 SnakeDollars to ${payerKeypair.publicKey}`, async () => {
            const snakeDollarUserATA = await createTokenAccount(program.provider.connection, payerKeypair, snakeDollarMintAddr, payerKeypair.publicKey);

            const [mintAuthAddress, _mintAuthBump] = anchor.web3.PublicKey.findProgramAddressSync(
                [
                  Buffer.from(anchor.utils.bytes.utf8.encode("mint")),
                ],
                program.programId
            )
            //console.log('Mint auth addr is ', mintAuthAddress)
            //console.log('User SnakeDollar ATA Address is ', snakeDollarUserATA)
            //console.log('Initializer pubkey is ', payerKeypair.publicKey)
            //console.log('User SnakeDollar ATA Address is', snakeDollarUserATA)

            
            const tx = await program.methods.mintSnd().accounts({
                initializer: payerKeypair.publicKey,
                snakeDollarMint: snakeDollarMintAddr,
                userSndAta: snakeDollarUserATA.address,
                snakeMintAuth: mintAuthAddress
            }).signers([payerKeypair]).rpc()    
            console.log("Mint snake dollars transaction signature", tx);
          });

          it(`Minting 0.2 SnakeBTC to ${payerKeypair.publicKey}`, async () => {
            const snakeBTCUserATA = await createTokenAccount(program.provider.connection, payerKeypair, snakeBTCMintAddr, payerKeypair.publicKey);

            const [mintAuthAddress, _mintAuthBump] = anchor.web3.PublicKey.findProgramAddressSync(
                [
                  Buffer.from(anchor.utils.bytes.utf8.encode("mint")),
                ],
                program.programId
            )

            
            const tx = await program.methods.mintSnbtc().accounts({
                initializer: payerKeypair.publicKey,
                snakeBtcMint: snakeBTCMintAddr,
                userSnbtcAta: snakeBTCUserATA.address,
                snakeMintAuth: mintAuthAddress
            }).signers([payerKeypair]).rpc()    
            console.log("Mint snake BTC transaction signature", tx);
          });

    }
});
