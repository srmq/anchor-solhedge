import * as anchor from "@coral-xyz/anchor";
import { AnchorSolhedge } from "../target/types/anchor_solhedge";
import { getAssociatedTokenAddress, Account } from "@solana/spl-token"
import { bs58 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import * as token from "@solana/spl-token"
import { Connection } from "@solana/web3.js";
import * as borsh from "borsh";

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


export const getMakerNextPutOptionVaultIdFromTx = async (
  program: anchor.Program<AnchorSolhedge>, 
  connection: Connection, 
  txid: string
): Promise<anchor.BN> => {
      //inspired by example in https://github.com/coral-xyz/anchor/blob/master/tests/cpi-returns/tests/cpi-return.ts
      let t = await connection.getTransaction(txid, {
        maxSupportedTransactionVersion: 0,
        commitment: "confirmed",
      });
      const [key, , buffer] = getReturnLog(t)
      if(key != program.programId) {
        throw new Error("Transaction is from another program")
      }
      const reader = new borsh.BinaryReader(buffer)
      const vaultNumber = reader.readU64()
      return vaultNumber
}

export const getCallOptionVaultFactoryPdaAddress = async (
  program: anchor.Program<AnchorSolhedge>,
  baseAssetMint: anchor.web3.PublicKey,
  quoteAssetMint: anchor.web3.PublicKey,
  maturity: anchor.BN,
  strike: anchor.BN
) => {

  const [callOptionVaultFactoryInfo, _callOptionVaultFactoryInfoBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from(anchor.utils.bytes.utf8.encode("CallOptionVaultFactoryInfo")),
      baseAssetMint.toBuffer(),
      quoteAssetMint.toBuffer(),
      maturity.toArrayLike(Buffer, "le", 8),
      strike.toArrayLike(Buffer, "le", 8)
    ],
    program.programId
  )

  return callOptionVaultFactoryInfo
}


export const getPutOptionVaultFactoryPdaAddress = async (
  program: anchor.Program<AnchorSolhedge>,
  baseAssetMint: anchor.web3.PublicKey,
  quoteAssetMint: anchor.web3.PublicKey,
  maturity: anchor.BN,
  strike: anchor.BN
) => {

  const [putOptionVaultFactoryInfo, _putOptionVaultFactoryInfoBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from(anchor.utils.bytes.utf8.encode("PutOptionVaultFactoryInfo")),
      baseAssetMint.toBuffer(),
      quoteAssetMint.toBuffer(),
      maturity.toArrayLike(Buffer, "le", 8),
      strike.toArrayLike(Buffer, "le", 8)
    ],
    program.programId
  )

  return putOptionVaultFactoryInfo
}

export const getPutOptionVaultDerivedPdaAddresses = async (
  program: anchor.Program<AnchorSolhedge>,
  vaultFactoryInfo: anchor.web3.PublicKey,
  baseAssetMint: anchor.web3.PublicKey,
  quoteAssetMint: anchor.web3.PublicKey,
  vaultId: anchor.BN
) => {

  const [putOptionVaultAddress, _putOptionVaultBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from(anchor.utils.bytes.utf8.encode("PutOptionVaultInfo")),
      vaultFactoryInfo.toBuffer(),
      vaultId.toArrayLike(Buffer, "le", 8)
    ],
    program.programId
  )


  const vaultBaseAssetTreasury = await getAssociatedTokenAddress(baseAssetMint, putOptionVaultAddress, true)
  const vaultQuoteAssetTreasury = await getAssociatedTokenAddress(quoteAssetMint, putOptionVaultAddress, true)

  return { putOptionVaultAddress, vaultBaseAssetTreasury, vaultQuoteAssetTreasury }
}

export const getCallOptionVaultDerivedPdaAddresses = async (
  program: anchor.Program<AnchorSolhedge>,
  vaultFactoryInfo: anchor.web3.PublicKey,
  baseAssetMint: anchor.web3.PublicKey,
  quoteAssetMint: anchor.web3.PublicKey,
  vaultId: anchor.BN
) => {

  const [callOptionVaultAddress, _callOptionVaultBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from(anchor.utils.bytes.utf8.encode("CallOptionVaultInfo")),
      vaultFactoryInfo.toBuffer(),
      vaultId.toArrayLike(Buffer, "le", 8)
    ],
    program.programId
  )


  const vaultBaseAssetTreasury = await getAssociatedTokenAddress(baseAssetMint, callOptionVaultAddress, true)
  const vaultQuoteAssetTreasury = await getAssociatedTokenAddress(quoteAssetMint, callOptionVaultAddress, true)

  return { callOptionVaultAddress: callOptionVaultAddress, vaultBaseAssetTreasury, vaultQuoteAssetTreasury }
}


export const getUserSettleTicketAccountAddressForPutVaultFactory = async (
  program: anchor.Program<AnchorSolhedge>,
  vaultFactoryInfo: anchor.web3.PublicKey,
  user: anchor.web3.PublicKey
) => {
  const [ticketAccountAddress, _ticketAccountBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from(anchor.utils.bytes.utf8.encode("PutOptionSettlePriceTicketInfo")),
      vaultFactoryInfo.toBuffer(),
      user.toBuffer()
    ],
    program.programId
  )
  return ticketAccountAddress
}

export const getUserTicketAccountAddressForCallVaultFactory = async (
  program: anchor.Program<AnchorSolhedge>,
  vaultFactoryInfo: anchor.web3.PublicKey,
  user: anchor.web3.PublicKey
) => {
  const [ticketAccountAddress, _ticketAccountBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from(anchor.utils.bytes.utf8.encode("CallOptionUpdateTicketInfo")),
      vaultFactoryInfo.toBuffer(),
      user.toBuffer()
    ],
    program.programId
  )
  return ticketAccountAddress
}


export const getUserTicketAccountAddressForPutVaultFactory = async (
  program: anchor.Program<AnchorSolhedge>,
  vaultFactoryInfo: anchor.web3.PublicKey,
  user: anchor.web3.PublicKey
) => {
  const [ticketAccountAddress, _ticketAccountBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from(anchor.utils.bytes.utf8.encode("PutOptionUpdateTicketInfo")),
      vaultFactoryInfo.toBuffer(),
      user.toBuffer()
    ],
    program.programId
  )
  return ticketAccountAddress
}

export const getPutMakerVaultAssociatedAccountAddress = async (
  program: anchor.Program<AnchorSolhedge>,
  vaultFactoryInfo: anchor.web3.PublicKey,
  vaultId: anchor.BN,
  user: anchor.web3.PublicKey
) => {
  const [userAssociatedAccountAddress, _UserAssociatedAccountBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from(anchor.utils.bytes.utf8.encode("PutOptionMakerInfo")),
      vaultFactoryInfo.toBuffer(),
      vaultId.toArrayLike(Buffer, "le", 8),
      user.toBuffer()
    ],
    program.programId
  )
  return userAssociatedAccountAddress
}

export const getAllMaybeNotMaturedPutFactories = async (
  program: anchor.Program<AnchorSolhedge>,
) => {
  const filter = [
    {
      memcmp: {
        offset: 8 + // Discriminator
                1 + // is_initialized
                8 + // next_vault_id
                8,  // maturity
        bytes: bs58.encode(Buffer.from([0]))
      },
    },
  ]
  const res = program.account.putOptionVaultFactoryInfo.all(filter)
  return res
}

export const getAllMaybeNotMaturedCallFactories = async (
  program: anchor.Program<AnchorSolhedge>,
) => {
  const filter = [
    {
      memcmp: {
        offset: 8 + // Discriminator
                1 + // is_initialized
                8 + // next_vault_id
                8,  // maturity
        bytes: bs58.encode(Buffer.from([0]))
      },
    },
  ]
  const res = program.account.callOptionVaultFactoryInfo.all(filter)
  return res
}


export const getVaultsForPutFactory = async (
  program: anchor.Program<AnchorSolhedge>,
  vaultFactoryAddress: anchor.web3.PublicKey,
) => {
  const filter = [
    {
      memcmp: {
        offset: 8, // Discriminator
        bytes: vaultFactoryAddress.toBase58()
      },
    },
  ]
  const res = program.account.putOptionVaultInfo.all(filter)
  return res
}

export const getVaultsForCallFactory = async (
  program: anchor.Program<AnchorSolhedge>,
  vaultFactoryAddress: anchor.web3.PublicKey,
) => {
  const filter = [
    {
      memcmp: {
        offset: 8, // Discriminator
        bytes: vaultFactoryAddress.toBase58()
      },
    },
  ]
  const res = program.account.callOptionVaultInfo.all(filter)
  return res
}


export const getUserMakerInfoAllPutVaults = async(
  program: anchor.Program<AnchorSolhedge>,
  userAddress: anchor.web3.PublicKey,
) => {

  const filter = [
    {
      memcmp: {
        offset: 8 + // Discriminator
                2 + // ord: u16
                8 + // quote_asset_qty: u64
                8 + // volume_sold: u64
                1 + // is_all_sold: bool,
                1 + // is_settled: bool
                8, // premium_limit: u64
        bytes: userAddress.toBase58()
      },
    },
  ]
  const res = program.account.putOptionMakerInfo.all(filter)
  return res

}

export const getUserMakerInfoAllCallVaults = async(
  program: anchor.Program<AnchorSolhedge>,
  userAddress: anchor.web3.PublicKey,
) => {

  const filter = [
    {
      memcmp: {
        offset: 8 + // Discriminator
                2 + // ord: u16
                8 + // quote_asset_qty: u64
                8 + // volume_sold: u64
                1 + // is_all_sold: bool,
                1 + // is_settled: bool
                8, // premium_limit: u64
        bytes: userAddress.toBase58()
      },
    },
  ]
  const res = program.account.callOptionMakerInfo.all(filter)
  return res

}


export const getUserTakerInfoAllPutVaults = async(
  program: anchor.Program<AnchorSolhedge>,
  userAddress: anchor.web3.PublicKey,
) => {

  const filter = [
    {
      memcmp: {
        offset: 8 + // Discriminator
                1 + // is_initialized: bool,
                2 + // ord: u16
                8 + // max_base_asset: u64
                8 + // qty_deposited: u64
                1, // is_settled: bool
        bytes: userAddress.toBase58()
      },
    },
  ]
  const res = program.account.putOptionTakerInfo.all(filter)
  return res

}


export const getUserTakerInfoForPutVault = async(
  program: anchor.Program<AnchorSolhedge>,
  vaultAddress: anchor.web3.PublicKey,
  userAddress: anchor.web3.PublicKey,
) => {

  const filter = [
    {
      memcmp: {
        offset: 8 + // Discriminator
                1 + // is_initialized: bool
                2 + // ord: u16
                8 + // max_base_asset: u64
                8 + // qty_deposited: u64
                1 + // is_settled: bool
                32, // owner: Pubkey
        bytes: vaultAddress.toBase58()
      },
    },
    {
      memcmp: {
        offset: 8 + // Discriminator
                1 + // is_initialized: bool
                2 + // ord: u16
                8 + // max_base_asset: u64
                8 + // qty_deposited: u64
                1, // is_settled: bool
        bytes: userAddress.toBase58()
      },
    }
  ]
  const res = program.account.putOptionTakerInfo.all(filter)

  return res
}


export const getUserMakerInfoForPutVault = async(
  program: anchor.Program<AnchorSolhedge>,
  vaultAddress: anchor.web3.PublicKey,
  userAddress: anchor.web3.PublicKey,
) => {

  const filter = [
    {
      memcmp: {
        offset: 8 + // Discriminator
                2 + // ord: u16
                8 + // quote_asset_qty: u64
                8 + // volume_sold: u64
                1 + // is_all_sold: bool,
                1 + // is_settled: bool
                8 + // premium_limit: u64
                32, // owner: Pubkey
        bytes: vaultAddress.toBase58()
      },
    },
    {
      memcmp: {
        offset: 8 + // Discriminator
                2 + // ord: u16
                8 + // quote_asset_qty: u64
                8 + // volume_sold: u64
                1 + // is_all_sold: bool,
                1 + // is_settled: bool
                8, // premium_limit: u64
        bytes: userAddress.toBase58()
      },
    }
  ]
  const res = program.account.putOptionMakerInfo.all(filter)

  return res
}

export const getUserMakerInfoForCallVault = async(
  program: anchor.Program<AnchorSolhedge>,
  vaultAddress: anchor.web3.PublicKey,
  userAddress: anchor.web3.PublicKey,
) => {

  const filter = [
    {
      memcmp: {
        offset: 8 + // Discriminator
                2 + // ord: u16
                8 + // base_asset_qty: u64
                8 + // volume_sold: u64
                1 + // is_all_sold: bool,
                1 + // is_settled: bool
                8 + // premium_limit: u64
                32, // owner: Pubkey
        bytes: vaultAddress.toBase58()
      },
    },
    {
      memcmp: {
        offset: 8 + // Discriminator
                2 + // ord: u16
                8 + // base_asset_qty: u64
                8 + // volume_sold: u64
                1 + // is_all_sold: bool,
                1 + // is_settled: bool
                8, // premium_limit: u64
        bytes: userAddress.toBase58()
      },
    }
  ]
  const res = program.account.callOptionMakerInfo.all(filter)

  return res
}


export const getPutSellersAsRemainingAccounts = async (
  wantedLots: number,
  program: anchor.Program<AnchorSolhedge>,
  sellers: PutOptionMakerInfo[],
  vaultAccount?: any,
  vaultFactoryAccount?: any
) => {
  if (vaultAccount == undefined) {
    const vaultAddr = sellers[0].account.putOptionVault
    vaultAccount = await program.account.putOptionVaultInfo.fetch(vaultAddr)  
  }
  if (vaultFactoryAccount == undefined) {
    const vaultFactoryAddr = vaultAccount.factoryVault
    vaultFactoryAccount = await program.account.putOptionVaultFactoryInfo.fetch(vaultFactoryAddr)  
  }
  const mint = vaultFactoryAccount.quoteAsset
  let sellersAndATAS = await getPutMakerATAs(program, sellers, mint)
  const quoteAssetByLot = (10**vaultAccount.lotSize)*vaultFactoryAccount.strike.toNumber()
  const lotsInQuoteAsset = wantedLots*quoteAssetByLot
  console.log(`${wantedLots} lots of ${10**vaultAccount.lotSize} at strike price ${vaultFactoryAccount.strike.toNumber()} mean ${lotsInQuoteAsset} in quote asset lamports`)
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
      if (potentialLots + userPotentialLots >= wantedLots) {
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
      if (potentialLots + userPotentialLots < wantedLots) {
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
  return remainingAccounts
}

export const getCallSellersAsRemainingAccounts = async (
  wantedLots: number,
  program: anchor.Program<AnchorSolhedge>,
  sellers: CallOptionMakerInfo[],
  vaultAccount?: any,
  vaultFactoryAccount?: any
) => {
  if (vaultAccount == undefined) {
    const vaultAddr = sellers[0].account.callOptionVault
    vaultAccount = await program.account.callOptionVaultInfo.fetch(vaultAddr)  
  }
  if (vaultFactoryAccount == undefined) {
    const vaultFactoryAddr = vaultAccount.factoryVault
    vaultFactoryAccount = await program.account.callOptionVaultFactoryInfo.fetch(vaultFactoryAddr)  
  }
  const mint = vaultFactoryAccount.quoteAsset
  const conn = program.provider.connection
  const baseMintInfo = await token.getMint(conn, vaultFactoryAccount.baseAsset)
  let sellersAndATAS = await getCallMakerATAs(program, sellers, mint)
  const baseAssetByLot = (10**vaultAccount.lotSize)*(10**baseMintInfo.decimals)
  //const lotsInQuoteAsset = wantedLots*quoteAssetByLot
  //console.log(`${wantedLots} lots of ${10**vaultAccount.lotSize} at strike price ${vaultFactoryAccount.strike.toNumber()} mean ${lotsInQuoteAsset} in quote asset lamports`)
  // will we get the first 4, and the 5st may be one later if the fourth does not complete
  // enough demand
  var i = 0
  let remainingAccounts = []
  for (const [callOptionMakerInfo, makerATA] of sellersAndATAS) {
    let potentialLots = 0
    if (i < 4) {
      const remAccountInfo = {
        pubkey: callOptionMakerInfo.publicKey,
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
      const baseAssetAvailable = callOptionMakerInfo.account.baseAssetQty.toNumber() - callOptionMakerInfo.account.volumeSold.toNumber()
      const userPotentialLots = Math.floor(baseAssetAvailable/baseAssetByLot)
      potentialLots += userPotentialLots
      console.log(`Call seller ${i} has at most ${userPotentialLots} lots to sell`)
    } else if (remainingAccounts.length >= 5) {
      break;
    } else if (i < sellersAndATAS.length-1){
      const baseAssetAvailable = callOptionMakerInfo.account.baseAssetQty.toNumber() - callOptionMakerInfo.account.volumeSold.toNumber()
      const userPotentialLots = Math.floor(baseAssetAvailable/baseAssetByLot)
      if (potentialLots + userPotentialLots >= wantedLots) {
        const remAccountInfo = {
          pubkey: callOptionMakerInfo.publicKey,
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
      let baseAssetAvailable = callOptionMakerInfo.account.baseAssetQty.toNumber() - callOptionMakerInfo.account.volumeSold.toNumber()
      let userPotentialLots = Math.floor(baseAssetAvailable/baseAssetByLot)
      let makerPubkey = callOptionMakerInfo.publicKey
      let ataPubkey = makerATA.address
      if (potentialLots + userPotentialLots < wantedLots) {
        makerPubkey = sellersAndATAS[4][0].publicKey
        ataPubkey = sellersAndATAS[4][1].address
        baseAssetAvailable = sellersAndATAS[4][0].account.baseAssetQty.toNumber() - sellersAndATAS[4][0].account.volumeSold.toNumber()
        userPotentialLots = Math.floor(baseAssetAvailable/baseAssetByLot)
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
  return remainingAccounts
}

export const getCallMakerATAs = async (
  program: anchor.Program<AnchorSolhedge>,
  sellers: CallOptionMakerInfo[],
  mint: anchor.web3.PublicKey
): Promise<Array<[CallOptionMakerInfo, Account]>> => {
  let conn = program.provider.connection
  let result: Array<[CallOptionMakerInfo, Account]> = []
  for (const seller of sellers) {
    let sellerATAAddress = await token.getAssociatedTokenAddress(mint, seller.account.owner, false)
    //verify if the account exist, we will not pay for its creation if not, just skip seller
    try {
      let sellerATA = await token.getAccount(conn, sellerATAAddress)
      //console.log('SELLER ATA')
      //console.log(sellerATA)
      if(sellerATA != null && "amount" in sellerATA) {
        result.push([seller, sellerATA])
      }  
    } catch(e) {
      console.log(`Looks like call seller ${seller.account.owner} does not have a ${mint} ATA, skipping... Error below`)
      console.log(e)
    }
  }
  return result
}


export const getPutMakerATAs = async (
  program: anchor.Program<AnchorSolhedge>,
  sellers: PutOptionMakerInfo[],
  mint: anchor.web3.PublicKey
): Promise<Array<[PutOptionMakerInfo, Account]>> => {
  let conn = program.provider.connection
  let result: Array<[PutOptionMakerInfo, Account]> = []
  for (const seller of sellers) {
    let sellerATAAddress = await token.getAssociatedTokenAddress(mint, seller.account.owner, false)
    //verify if the account exist, we will not pay for its creation if not, just skip seller
    let sellerATA = await token.getAccount(conn, sellerATAAddress)
    //console.log('SELLER ATA')
    //console.log(sellerATA)
    if(sellerATA != null && "amount" in sellerATA) {
      result.push([seller, sellerATA])
    }
  }
  return result
}

export const getCallSellersInVault = async (
  program: anchor.Program<AnchorSolhedge>,
  vaultAddress: anchor.web3.PublicKey,
  fairPrice: number,
  slippageTolerance: number
): Promise<CallOptionMakerInfo[]> => {
  console.log(`fairPrice in getCallSellersInVault is ${fairPrice}`)
  if (!slippageTolerance || slippageTolerance <= 0.0) {
    throw new Error(`slippageTolerance has to be correctly defined, cannot be ${slippageTolerance}`)
  }
  if (!fairPrice || fairPrice <= 0.0) {
    throw new Error(`fairPrice should be greater than zero, cannot be ${fairPrice}`)
  }
  if (!Number.isSafeInteger(fairPrice)) {
    throw new Error(`fairPrice should be an integer in price lamports, cannot be ${fairPrice}`)
  }

  let chainResults = await getAllCallMakerInfosForVault(program, vaultAddress)
  chainResults = chainResults.filter(makerInfo => 
    !makerInfo.account.isAllSold
    && makerInfo.account.premiumLimit.toNumber() <= Math.floor((1.0+slippageTolerance)*fairPrice))
  
  chainResults.sort((a, b) => (a.account.ord as number) - (b.account.ord as number))
  let result: CallOptionMakerInfo[] = []
  chainResults.forEach(chainResult => {
    result.push(new CallOptionMakerInfo(chainResult))
  });
  return result
}


export const getPutSellersInVault = async (
  program: anchor.Program<AnchorSolhedge>,
  vaultAddress: anchor.web3.PublicKey,
  fairPrice: number,
  slippageTolerance: number
): Promise<PutOptionMakerInfo[]> => {
  console.log(`fairPrice in getSellersInVault is ${fairPrice}`)
  if (!slippageTolerance || slippageTolerance <= 0.0) {
    throw new Error(`slippageTolerance has to be correctly defined, cannot be ${slippageTolerance}`)
  }
  if (!fairPrice || fairPrice <= 0.0) {
    throw new Error(`fairPrice should be greater than zero, cannot be ${fairPrice}`)
  }
  if (!Number.isSafeInteger(fairPrice)) {
    throw new Error(`fairPrice should be an integer in price lamports, cannot be ${fairPrice}`)
  }

  let chainResults = await getAllPutMakerInfosForVault(program, vaultAddress)
  chainResults = chainResults.filter(makerInfo => 
    !makerInfo.account.isAllSold
    && makerInfo.account.premiumLimit.toNumber() <= Math.floor((1.0+slippageTolerance)*fairPrice))
  
  chainResults.sort((a, b) => (a.account.ord as number) - (b.account.ord as number))
  let result: PutOptionMakerInfo[] = []
  chainResults.forEach(chainResult => {
    result.push(new PutOptionMakerInfo(chainResult))
  });
  return result
}

export class PutOptionMakerInfo {
  publicKey: anchor.web3.PublicKey
  account: {
    ord: number
    quoteAssetQty: anchor.BN
    volumeSold: anchor.BN
    isAllSold: boolean
    isSettled: boolean
    premiumLimit: anchor.BN
    owner: anchor.web3.PublicKey
    putOptionVault: anchor.web3.PublicKey
  }

  constructor(params: {
    publicKey: anchor.web3.PublicKey
    account: {
      ord: number | anchor.BN
      quoteAssetQty: anchor.BN
      volumeSold: anchor.BN
      isAllSold: boolean
      isSettled: boolean
      premiumLimit: anchor.BN
      owner: anchor.web3.PublicKey
      putOptionVault: anchor.web3.PublicKey
    }    
  }){
    this.publicKey = params.publicKey
    this.account = {
      ord: new anchor.BN(params.account.ord).toNumber(),
      quoteAssetQty: params.account.quoteAssetQty,
      volumeSold: params.account.volumeSold,
      isAllSold: params.account.isAllSold,
      isSettled: params.account.isSettled,
      premiumLimit: params.account.premiumLimit,
      owner: params.account.owner,
      putOptionVault: params.account.putOptionVault
    }
  }
}

export class CallOptionMakerInfo {
  publicKey: anchor.web3.PublicKey
  account: {
    ord: number
    baseAssetQty: anchor.BN
    volumeSold: anchor.BN
    isAllSold: boolean
    isSettled: boolean
    premiumLimit: anchor.BN
    owner: anchor.web3.PublicKey
    callOptionVault: anchor.web3.PublicKey
  }

  constructor(params: {
    publicKey: anchor.web3.PublicKey
    account: {
      ord: number | anchor.BN
      baseAssetQty: anchor.BN
      volumeSold: anchor.BN
      isAllSold: boolean
      isSettled: boolean
      premiumLimit: anchor.BN
      owner: anchor.web3.PublicKey
      callOptionVault: anchor.web3.PublicKey
    }    
  }){
    this.publicKey = params.publicKey
    this.account = {
      ord: new anchor.BN(params.account.ord).toNumber(),
      baseAssetQty: params.account.baseAssetQty,
      volumeSold: params.account.volumeSold,
      isAllSold: params.account.isAllSold,
      isSettled: params.account.isSettled,
      premiumLimit: params.account.premiumLimit,
      owner: params.account.owner,
      callOptionVault: params.account.callOptionVault
    }
  }
}



export const getAllPutMakerInfosForVault = async(
  program: anchor.Program<AnchorSolhedge>,
  vaultAddress: anchor.web3.PublicKey,
) => {

  const filter = [
    {
      memcmp: {
        offset: 8 + // Discriminator
                2 + // ord: u16
                8 + // quote_asset_qty: u64
                8 + // volume_sold: u64
                1 + // is_all_sold: bool,                
                1 + // is_settled: bool
                8 + // premium_limit: u64
                32, // owner: Pubkey
        bytes: vaultAddress.toBase58()
      },
    },
  ]
  const res = program.account.putOptionMakerInfo.all(filter)
  return res
}

export const getAllCallMakerInfosForVault = async(
  program: anchor.Program<AnchorSolhedge>,
  vaultAddress: anchor.web3.PublicKey,
) => {

  const filter = [
    {
      memcmp: {
        offset: 8 + // Discriminator
                2 + // ord: u16
                8 + // base_asset_qty: u64
                8 + // volume_sold: u64
                1 + // is_all_sold: bool,                
                1 + // is_settled: bool
                8 + // premium_limit: u64
                32, // owner: Pubkey
        bytes: vaultAddress.toBase58()
      },
    },
  ]
  const res = program.account.callOptionMakerInfo.all(filter)
  return res
}



export class MakerCreatePutOptionParams {
  maturity: anchor.BN //u64,
  strike: anchor.BN //u64,
  maxMakers: number //u16,
  maxTakers: number //u16,
  lotSize: number //i8,
  numLotsToSell: anchor.BN //u64,
  premiumLimit: anchor.BN //u64

  constructor(params: {
    maturity: anchor.BN //u64,
    strike: anchor.BN //u64,
    maxMakers: number //u16,
    maxTakers: number //u16,
    lotSize: number //i8,
    numLotsToSell: anchor.BN //u64,
    premiumLimit: anchor.BN //u64
  }) {
    this.maturity = params.maturity
    this.strike = params.strike
    this.maxMakers = params.maxMakers
    this.maxTakers = params.maxTakers
    this.lotSize = params.lotSize
    this.numLotsToSell = params.numLotsToSell
    this.premiumLimit = params.premiumLimit
  }
}

export class MakerCreateCallOptionParams {
  maturity: anchor.BN //u64,
  strike: anchor.BN //u64,
  maxMakers: number //u16,
  maxTakers: number //u16,
  lotSize: number //i8,
  numLotsToSell: anchor.BN //u64,
  premiumLimit: anchor.BN //u64

  constructor(params: {
    maturity: anchor.BN //u64,
    strike: anchor.BN //u64,
    maxMakers: number //u16,
    maxTakers: number //u16,
    lotSize: number //i8,
    numLotsToSell: anchor.BN //u64,
    premiumLimit: anchor.BN //u64
  }) {
    this.maturity = params.maturity
    this.strike = params.strike
    this.maxMakers = params.maxMakers
    this.maxTakers = params.maxTakers
    this.lotSize = params.lotSize
    this.numLotsToSell = params.numLotsToSell
    this.premiumLimit = params.premiumLimit
  }
}