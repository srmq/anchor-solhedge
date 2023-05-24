import * as anchor from "@coral-xyz/anchor";
import { AnchorSolhedge } from "../target/types/anchor_solhedge";
import { getAssociatedTokenAddress } from "@solana/spl-token"
import { bs58 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";

export const getVaultFactoryPdaAddress = async (
  program: anchor.Program<AnchorSolhedge>,
  baseAssetMint: anchor.web3.PublicKey,
  quoteAssetMint: anchor.web3.PublicKey,
  params: MakerCreatePutOptionParams
) => {

  const [putOptionVaultFactoryInfo, _putOptionVaultFactoryInfoBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      Buffer.from(anchor.utils.bytes.utf8.encode("PutOptionVaultFactoryInfo")),
      baseAssetMint.toBuffer(),
      quoteAssetMint.toBuffer(),
      params.maturity.toArrayLike(Buffer, "le", 8),
      params.strike.toArrayLike(Buffer, "le", 8)
    ],
    program.programId
  )

  return putOptionVaultFactoryInfo
}

export const getVaultDerivedPdaAddresses = async (
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

export const getUserVaultAssociatedAccountAddress = async (
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

export const getAllMaybeNotMaturedFactories = async (
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

export const getUserMakerInfoAllVaults = async(
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
                1 + // is_settled: bool
                8, // premium_limit: u64
        bytes: userAddress.toBase58()
      },
    },
  ]
  const res = program.account.putOptionMakerInfo.all(filter)
  return res

}


export class MakerCreatePutOptionParams {
  maturity: anchor.BN //u64,
  strike: anchor.BN //u64,
  maxMakers: number //u16,
  maxTakers: number //u16,
  lotSize: anchor.BN //u64,
  minTickerIncrement: number //f32,
  numLotsToSell: anchor.BN //u64,
  premiumLimit: anchor.BN //u64

  constructor(params: {
    maturity: anchor.BN //u64,
    strike: anchor.BN //u64,
    maxMakers: number //u16,
    maxTakers: number //u16,
    lotSize: anchor.BN //u64,
    minTickerIncrement: number //f32,
    numLotsToSell: anchor.BN //u64,
    premiumLimit: anchor.BN //u64
  }) {
    this.maturity = params.maturity
    this.strike = params.strike
    this.maxMakers = params.maxMakers
    this.maxTakers = params.maxTakers
    this.lotSize = params.lotSize
    this.minTickerIncrement = params.minTickerIncrement
    this.numLotsToSell = params.numLotsToSell
    this.premiumLimit = params.premiumLimit
  }
}