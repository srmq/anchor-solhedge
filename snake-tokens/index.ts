import * as web3 from "@solana/web3.js";
import { initializeKeypair } from "./initializeKeypair";
import * as token from "@solana/spl-token";
import {
    bundlrStorage,
    keypairIdentity,
    Metaplex,
    toMetaplexFile,
} from "@metaplex-foundation/js";
import {
    DataV2,
    createCreateMetadataAccountV3Instruction
} from "@metaplex-foundation/mpl-token-metadata";

import * as fs from "fs";


const ASSETS_PATH: string = 'snake-tokens/assets/'
const TOKENS_PATH: string = 'snake-tokens/'

class MockData {
    name: string
    symbol: string
    description: string
    imageName: string
    decimals: number
}

const mockTokens : Array<MockData> = [
    {
        name: 'SnakeBTC',
        symbol: 'snBTC',
        description: 'A mock token for wrapped bitcoin',
        imageName: 'SnakeBTC512.png',
        decimals: 8
    },
    {
        name: 'SnakeDollar',
        symbol: 'snD',
        description: 'A mock token for the dollar',
        imageName: 'SnakeDollar512.png',
        decimals: 6
    }
]

async function createMockToken(
    connection: web3.Connection,
    payer: web3.Keypair,
    programId: web3.PublicKey,
    mockData: MockData
) {
    const [mintAuth] = web3.PublicKey.findProgramAddressSync(
        [Buffer.from("mint")],
        programId
    )
    console.log("found program that will be mint authority")    
    // This will create a token with all the necessary inputs
    var tokenMint
    try {
        tokenMint = await token.createMint(
            connection, // Connection
            payer, // Payer
            payer.publicKey, // Your wallet public key (mint authority)
            payer.publicKey, // Freeze authority
            mockData.decimals // Decimals
        );    
    } catch (e) {
        console.log(e)
    }
    console.log("created token mint")    

    // Create a metaplex object so that we can create a metaplex metadata
    const metaplex = Metaplex.make(connection)
        .use(keypairIdentity(payer))
        .use(
            bundlrStorage({
                address: "https://devnet.bundlr.network",
                providerUrl: "https://api.devnet.solana.com",
                timeout: 60000,
            })
        );

    // Read image file
    const imageBuffer = fs.readFileSync(ASSETS_PATH + mockData.imageName);
    const file = toMetaplexFile(imageBuffer, mockData.imageName);
    const imageUri = await metaplex.storage().upload(file);

    // Upload the rest of offchain metadata
    const { uri } = await metaplex
        .nfts()
        .uploadMetadata({
            name: mockData.name,
            description: mockData.description,
            image: imageUri,
        }, {commitment: "finalized"});
        console.log("token metadata uploaded")    

    // Finding out the address where the metadata is stored
    const metadataPda = metaplex.nfts().pdas().metadata({ mint: tokenMint });
    console.log("metadata pda found")    
    const tokenMetadata = {
        name: mockData.name,
        symbol: mockData.symbol,
        uri: uri,
        sellerFeeBasisPoints: 0,
        creators: null,
        collection: null,
        uses: null,
    } as DataV2

    const instruction = createCreateMetadataAccountV3Instruction({
        metadata: metadataPda,
        mint: tokenMint,
        mintAuthority: payer.publicKey,
        payer: payer.publicKey,
        updateAuthority: payer.publicKey
    },
        {
            createMetadataAccountArgsV3: {
                data: tokenMetadata,
                isMutable: true,
                collectionDetails: {
                    __kind: "V1",
                    size: 1
                }
            }
        })

    console.log("CreateMetadataAccountV3Instruction created")
    const transaction = new web3.Transaction()
    transaction.add(instruction)

    const transactionSignature = await web3.sendAndConfirmTransaction(
        connection,
        transaction,
        [payer]
    )
    console.log("created metadata account for token")

    await token.setAuthority(
        connection,
        payer,
        tokenMint,
        payer,
        token.AuthorityType.MintTokens,
        mintAuth
    )
    console.log("set token authority")

    fs.writeFileSync(
        TOKENS_PATH +  `${mockData.symbol}.json`,
        JSON.stringify({
          mint: tokenMint.toBase58(),
          imageUri: imageUri,
          metadataUri: uri,
          tokenMetadata: metadataPda.toBase58(),
          metadataTransaction: transactionSignature,
        })
      );

}


async function main() {
    const connection = new web3.Connection(web3.clusterApiUrl("devnet"), "confirmed");
    const payer = await initializeKeypair(connection);

    // this is the program id of snake-minter-devnet
    const mintAuthority = new web3.PublicKey("2dAPtThes6YDdLL7bHUMPSduce9rKmnobSP8fQ4X5yTS")
    for (let mockData of mockTokens) {
        await createMockToken(connection, payer, mintAuthority, mockData)
    }

}

main()
    .then(() => {
        console.log("Finished successfully");
        process.exit(0);
    })
    .catch((error) => {
        console.log(error);
        process.exit(1);
    });