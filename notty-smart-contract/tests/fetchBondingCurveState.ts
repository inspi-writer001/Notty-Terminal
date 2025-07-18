import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { NottySmartContract } from "../target/types/notty_smart_contract";

const TOTAL_SUPPLY = new anchor.BN("100000000000000000"); // 100M tokens with 9 decimals
const START_MARKET_CAP_SOL = new anchor.BN("10000000"); // 0.01 SOL ✅
const END_MARKET_CAP_SOL = new anchor.BN("1000000000"); // 1 SOL ✅

anchor.setProvider(anchor.AnchorProvider.env());

const program = anchor.workspace
  .NottySmartContract as Program<NottySmartContract>;

export async function fetchBondingCurveState(mintAddress) {
  console.log("\n🔍 FETCHING BONDING CURVE STATE");
  console.log("=".repeat(50));

  try {
    // 1. Derive the bonding vault PDA
    const [bondingVaultPDA] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("bonding_vault"), mintAddress.toBuffer()],
      program.programId
    );

    console.log("Mint Address:", mintAddress.toString());
    console.log("Bonding Vault PDA:", bondingVaultPDA.toString());

    // 2. Fetch the account data
    const vaultAccount = await program.account.bondingCurveVault.fetch(
      bondingVaultPDA
    );
    console.log("\n📊 RAW ACCOUNT DATA:");
    console.log("- Creator:", vaultAccount.creator.toString());
    console.log("- Total Supply:", vaultAccount.totalSupply.toString());
    console.log("- Tokens Sold:", vaultAccount.tokensSold.toString());
    console.log("- SOL Raised:", vaultAccount.solRaised.toString(), "lamports");
    console.log("- Graduated:", vaultAccount.graduated);
    console.log("- Migrated:", vaultAccount.migrated);

    // 3. Calculate metrics using FIXED LINEAR BONDING CURVE MATH
    const tokensSoldBN = new anchor.BN(vaultAccount.tokensSold.toString());
    const solRaisedBN = new anchor.BN(vaultAccount.solRaised.toString());
    const totalSupplyBN = new anchor.BN(vaultAccount.totalSupply.toString());

    // Progress through the bonding curve (0-100%)
    const progress = tokensSoldBN.mul(new anchor.BN(10000)).div(totalSupplyBN);
    const progressPercent = progress.toNumber() / 100;

    // FIXED: Calculate base price and slope (matching your Rust implementation)
    const basePrice = START_MARKET_CAP_SOL.mul(new anchor.BN(1000000000)).div(
      totalSupplyBN
    );
    const priceRange = END_MARKET_CAP_SOL.sub(START_MARKET_CAP_SOL);
    const slope = priceRange.mul(new anchor.BN(1000000000)).div(totalSupplyBN);

    // FIXED: Current token price using linear bonding curve
    let currentTokenPrice;
    if (tokensSoldBN.eq(new anchor.BN(0))) {
      currentTokenPrice = basePrice;
    } else {
      // price = base_price + slope * tokens_sold
      const priceIncrease = slope
        .mul(tokensSoldBN)
        .div(new anchor.BN(1000000000));
      currentTokenPrice = basePrice.add(priceIncrease);
    }

    // FIXED: Market cap = current_price * TOTAL_SUPPLY (not tokens_sold!)
    const currentMarketCap = currentTokenPrice
      .mul(totalSupplyBN)
      .div(new anchor.BN(1000000000));

    // How much SOL needed to graduate
    const solToGraduate = END_MARKET_CAP_SOL.sub(currentMarketCap);

    // Tokens remaining
    const tokensRemaining = totalSupplyBN.sub(tokensSoldBN);

    console.log("\n📈 CALCULATED METRICS (FIXED MATH):");
    console.log("- Base Price:", basePrice.toString(), "lamports per token");
    console.log("- Slope:", slope.toString(), "lamports per token per token");
    console.log("- Progress:", progressPercent.toFixed(4) + "%");
    console.log(
      "- Current Market Cap:",
      currentMarketCap.toString(),
      "lamports",
      `(${(currentMarketCap.toNumber() / 1000000000).toFixed(6)} SOL)`
    );
    console.log(
      "- Current Token Price:",
      currentTokenPrice.toString(),
      "lamports per token"
    );
    console.log(
      "- SOL to Graduate:",
      solToGraduate.toString(),
      "lamports",
      `(${(solToGraduate.toNumber() / 1000000000).toFixed(6)} SOL)`
    );
    console.log("- Tokens Remaining:", tokensRemaining.toString());
    console.log(
      "- SOL in Pool:",
      solRaisedBN.toString(),
      "lamports",
      `(${(solRaisedBN.toNumber() / 1000000000).toFixed(6)} SOL)`
    );

    // 4. Check graduation status
    console.log("\n🎯 GRADUATION STATUS:");
    if (vaultAccount.graduated) {
      console.log("✅ GRADUATED! Ready for migration to DEX");
    } else {
      const percentToGraduation =
        (currentMarketCap.toNumber() / END_MARKET_CAP_SOL.toNumber()) * 100;
      console.log(`📊 ${percentToGraduation.toFixed(2)}% to graduation`);
      console.log(
        `💰 Need ${(solToGraduate.toNumber() / 1000000000).toFixed(
          6
        )} more SOL to graduate`
      );
    }

    // 5. Get SOL vault balance
    const [solVaultPDA] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("sol_vault"), mintAddress.toBuffer()],
      program.programId
    );

    const solVaultBalance = await program.provider.connection.getBalance(
      solVaultPDA
    );
    console.log("\n💰 SOL VAULT BALANCE:");
    console.log(
      "- Balance:",
      solVaultBalance,
      "lamports",
      `(${(solVaultBalance / 1000000000).toFixed(6)} SOL)`
    );

    // 6. Get token vault balance
    const [tokenVaultPDA] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("token_vault"), mintAddress.toBuffer()],
      program.programId
    );

    const tokenVaultInfo =
      await program.provider.connection.getTokenAccountBalance(tokenVaultPDA);
    console.log("\n🪙 TOKEN VAULT BALANCE:");
    console.log("- Tokens in Vault:", tokenVaultInfo.value.amount);
    console.log("- Decimals:", tokenVaultInfo.value.decimals);

    // 7. BONUS: Estimate cost for next purchase
    console.log("\n💡 NEXT PURCHASE ESTIMATES:");
    const oneTokenCost = estimateBuyCost(
      tokensSoldBN,
      new anchor.BN(1000000000)
    ); // 1 token
    const hundredTokensCost = estimateBuyCost(
      tokensSoldBN,
      new anchor.BN(100000000000)
    ); // 100 tokens

    console.log("- Cost for 1 token:", oneTokenCost.toString(), "lamports");
    console.log(
      "- Cost for 100 tokens:",
      hundredTokensCost.toString(),
      "lamports"
    );

    return {
      vaultAccount,
      progress: progressPercent,
      currentMarketCap: currentMarketCap.toNumber(),
      currentTokenPrice: currentTokenPrice.toNumber(),
      solToGraduate: solToGraduate.toNumber(),
      tokensRemaining: tokensRemaining.toString(),
      solVaultBalance,
      tokenVaultBalance: tokenVaultInfo.value.amount
    };
  } catch (error) {
    console.error("❌ Error fetching bonding curve state:", error);
    throw error;
  }
}

// Helper function to estimate buy cost (matches your Rust implementation)
function estimateBuyCost(currentSupply, tokenAmount) {
  // Calculate base price and slope
  const basePrice = START_MARKET_CAP_SOL.mul(new anchor.BN(1000000000)).div(
    TOTAL_SUPPLY
  );
  const priceRange = END_MARKET_CAP_SOL.sub(START_MARKET_CAP_SOL);
  const slope = priceRange.mul(new anchor.BN(1000000000)).div(TOTAL_SUPPLY);

  const startTokens = currentSupply;
  const endTokens = startTokens.add(tokenAmount);

  // Integration: base_price * amount + slope * (end² - start²) / 2
  const baseCost = basePrice.mul(tokenAmount).div(new anchor.BN(1000000000));

  const endSquare = endTokens.mul(endTokens);
  const startSquare = startTokens.mul(startTokens);
  const squareDiff = endSquare.sub(startSquare);

  const slopeCost = slope
    .mul(squareDiff)
    .div(new anchor.BN(2).mul(new anchor.BN(1000000000)));

  return baseCost.add(slopeCost);
}
