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