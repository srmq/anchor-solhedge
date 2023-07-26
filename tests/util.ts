import {
	Connection,
	Keypair,
	PublicKey,
	TransactionInstruction,
	VersionedTransaction,
	TransactionMessage,
} from "@solana/web3.js";

export function keyPairFromSecret(secret: number[]): Keypair {
	const secretKey = Uint8Array.from(secret)
	const keypair = Keypair.fromSecretKey(secretKey)
	//console.log(keypair.publicKey.toString())
	return keypair
}
  

export const isLocalnet = (conn : Connection): boolean => {
	const ep = conn.rpcEndpoint.toLowerCase()
	return ep.startsWith("http://0.0.0.0") ||
	  ep.startsWith("http://localhost") ||
	  ep.startsWith("http://127.0.0.1")
}
  

export async function sendTransactionV0(
	connection: Connection,
	instructions: TransactionInstruction[],
	payer: Keypair,
): Promise<string> {
	let blockhash = await connection
		.getLatestBlockhash()
		.then((res) => res.blockhash);

	const messageV0 = new TransactionMessage({
		payerKey: payer.publicKey,
		recentBlockhash: blockhash,
		instructions,
	}).compileToV0Message();

	const tx = new VersionedTransaction(messageV0);
	tx.sign([payer]);
	const sx = await connection.sendTransaction(tx);

    return sx;
}

export async function sendTransactionV0WithLookupTable(
	connection: Connection,
	instructions: TransactionInstruction[],
	payer: Keypair,
	lookupTablePubkey: PublicKey,
): Promise<string> {
	const lookupTableAccount = await connection
		.getAddressLookupTable(lookupTablePubkey)
		.then((res) => res.value);

	let blockhash = await connection
		.getLatestBlockhash()
		.then((res) => res.blockhash);

	const messageV0 = new TransactionMessage({
		payerKey: payer.publicKey,
		recentBlockhash: blockhash,
		instructions,
	}).compileToV0Message([lookupTableAccount]);

	const tx = new VersionedTransaction(messageV0);
	tx.sign([payer]);
	const sx = await connection.sendTransaction(tx);

    return sx;
}

export async function printAddressLookupTable(
	connection: Connection,
	lookupTablePubkey: PublicKey,
): Promise<void> {
	const lookupTableAccount = await connection
		.getAddressLookupTable(lookupTablePubkey)
		.then((res) => res.value);
	console.log(`Lookup Table: ${lookupTablePubkey}`);
	for (let i = 0; i < lookupTableAccount.state.addresses.length; i++) {
		const address = lookupTableAccount.state.addresses[i];
		console.log(`   Index: ${i}  Address: ${address.toBase58()}`);
	}
}

export enum CandleGranularity {
    ONE_MIN = "ONE_MIN",
    FIVE_MIN = "FIVE_MIN",
    ONE_HOUR = "ONE_HOUR",
    ONE_DAY = "ONE_DAY",
    ONE_WEEK = "ONE_WEEK"
}

export const granularityToSeconds = (granularity: CandleGranularity): number => {
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



export class TokenPrice {
	mint: string
	price: number
	ts: number
  
	constructor(candle: {
	  mint: string
	  granularity: CandleGranularity
	  startTime: number
	  close: number
	}){
	  this.mint = candle.mint
	  this.price = candle.close
	  const addToStart = granularityToSeconds(candle.granularity)
	  this.ts = candle.startTime + addToStart
	}
  }
  