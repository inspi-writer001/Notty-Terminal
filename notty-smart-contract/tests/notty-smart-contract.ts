import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { NottySmartContract } from "../target/types/notty_smart_contract";

import admin_wallet_file from "./admin-wallet.json";
import test_user_1_file from "./tester-1-wallet.json";
import test_user_2_file from "./tester-2-wallet.json";

let admin_wallet = anchor.web3.Keypair.fromSecretKey(
  new Uint8Array(admin_wallet_file)
);
let test_user_1 = anchor.web3.Keypair.fromSecretKey(
  new Uint8Array(test_user_1_file)
);
let test_user_2 = anchor.web3.Keypair.fromSecretKey(
  new Uint8Array(test_user_2_file)
);

// Constants matching your Rust code
const TOTAL_SUPPLY = new anchor.BN("100000000000000000"); // 100M tokens with 9 decimals
const START_MARKET_CAP_SOL = new anchor.BN("10000000"); // 0.01 SOL ✅
const END_MARKET_CAP_SOL = new anchor.BN("1000000000"); // 1 SOL ✅

describe("notty-smart-contract", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace
    .NottySmartContract as Program<NottySmartContract>;

  // Helper function to fetch and analyze bonding curve state
  async function fetchBondingCurveState(mintAddress: anchor.web3.PublicKey) {
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
      console.log(
        "- SOL Raised:",
        vaultAccount.solRaised.toString(),
        "lamports"
      );
      console.log("- Graduated:", vaultAccount.graduated);
      console.log("- Migrated:", vaultAccount.migrated);

      // 3. Calculate useful metrics
      const tokensSoldBN = new anchor.BN(vaultAccount.tokensSold.toString());
      const solRaisedBN = new anchor.BN(vaultAccount.solRaised.toString());
      const totalSupplyBN = new anchor.BN(vaultAccount.totalSupply.toString());

      // Progress through the bonding curve (0-100%)
      const progress = tokensSoldBN
        .mul(new anchor.BN(10000))
        .div(totalSupplyBN); // Basis points
      const progressPercent = progress.toNumber() / 100;

      // Current market cap calculation
      let currentMarketCap;
      if (tokensSoldBN.eq(new anchor.BN(0))) {
        currentMarketCap = START_MARKET_CAP_SOL;
      } else {
        const progressRatio = tokensSoldBN
          .mul(new anchor.BN(1000000))
          .div(totalSupplyBN);
        const priceRange = END_MARKET_CAP_SOL.sub(START_MARKET_CAP_SOL);
        const marketCapIncrease = priceRange
          .mul(progressRatio)
          .div(new anchor.BN(1000000));
        currentMarketCap = START_MARKET_CAP_SOL.add(marketCapIncrease);
      }

      // Current token price (lamports per token)
      let currentTokenPrice;
      if (tokensSoldBN.eq(new anchor.BN(0))) {
        currentTokenPrice = START_MARKET_CAP_SOL.mul(
          new anchor.BN(1000000000)
        ).div(totalSupplyBN);
      } else {
        currentTokenPrice = currentMarketCap
          .mul(new anchor.BN(1000000000))
          .div(tokensSoldBN);
      }

      // How much SOL needed to graduate
      const solToGraduate = END_MARKET_CAP_SOL.sub(currentMarketCap);

      // Tokens remaining
      const tokensRemaining = totalSupplyBN.sub(tokensSoldBN);

      console.log("\n📈 CALCULATED METRICS:");
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

  //   it("Is initialized!", async () => {
  //     // Add your test here.
  //     const tx = await program.methods
  //       .initialize(admin_wallet.publicKey)
  //       .accounts({
  //         payer: admin_wallet.publicKey
  //       })
  //       .signers([admin_wallet])
  //       .rpc();
  //     console.log("Your transaction signature", tx);
  //   });

  //   it("should create bonding curve token", async () => {
  //     // Add your test here.

  //     let new_mint = anchor.web3.Keypair.generate();
  //     const tx = await program.methods
  //       .createBondingCurveToken(
  //         "Hello Token",
  //         "HTK",
  //         "https://fastly.picsum.photos/id/237/200/300.jpg?hmac=TmmQSbShHz9CdQm0NkEjx1Dyh_Y984R9LpNrpvH2D_U"
  //       )
  //       .signers([test_user_1, new_mint])
  //       .accounts({
  //         creator: test_user_1.publicKey,
  //         mintAccount: new_mint.publicKey
  //       })
  //       .rpc();

  //     console.log(new_mint);
  //     console.log("Your transaction signature", tx);
  //   });

  // it("should create bonding curve token", async () => {
  //   let new_mint = anchor.web3.Keypair.generate();

  //   try {
  //     const tx = await program.methods
  //       .createBondingCurveToken(
  //         "Hello Token",
  //         "HTK",
  //         "https://fastly.picsum.photos/id/237/200/300.jpg?hmac=TmmQSbShHz9CdQm0NkEjx1Dyh_Y984R9LpNrpvH2D_U"
  //       )
  //       .accounts({
  //         creator: test_user_1.publicKey,
  //         mintAccount: new_mint.publicKey
  //       })
  //       .signers([test_user_1, new_mint])
  //       .rpc();

  //     console.log("Your transaction signature", tx);
  //   } catch (error) {
  //     console.error("Full error:", error);
  //     if (error.logs) {
  //       console.error("Transaction logs:", error.logs);
  //     }
  //   }
  // });

  //   it("should allow admin withdraw funds from wallet", async () => {
  //     // Add your test here.

  //     let new_mint = anchor.web3.Keypair.generate();
  //     const tx = await program.methods
  //       .withdrawFees(new anchor.BN(1_000_000))
  //       .signers([admin_wallet])
  //       .accounts({
  //         admin: admin_wallet.publicKey
  //       })
  //       .rpc();

  //     console.log(new_mint);
  //     console.log("Your transaction signature", tx);
  //   });

  //   it("should fail to allow non-admin withdraw funds from wallet", async () => {
  //     // Add your test here.

  //     let new_mint = anchor.web3.Keypair.generate();
  //     const tx = await program.methods
  //       .withdrawFees(new anchor.BN(1_000_000))
  //       .signers([test_user_1])
  //       .accounts({
  //         admin: test_user_1.publicKey
  //       })
  //       .rpc();

  //     console.log(new_mint);
  //     console.log("Your transaction signature", tx);
  //   });

  //   it("should create token and buy from bonding curve", async () => {
  //     // Step 1: Create a new bonding curve token
  //     const newMint = anchor.web3.Keypair.generate();

  //     const createTx = await program.methods
  //       .createBondingCurveToken(
  //         "H3llo Tok3n",
  //         "H3K",
  //         "https://fastly.picsum.photos/id/237/200/300.jpg?hmac=TmmQSbShHz9CdQm0NkEjx1Dyh_Y984R9LpNrpvH2D_U"
  //       )
  //       .signers([test_user_1, newMint])
  //       .accounts({
  //         creator: test_user_1.publicKey,
  //         mintAccount: newMint.publicKey
  //       })
  //       .rpc();

  //     console.log("Token created:", createTx);

  //     // Step 2: Buy tokens from the bonding curve
  //     const buyTx = await program.methods
  //       .buyTokensBondingCurve(
  //         new anchor.BN(100_000_000), // 0.1 SOL
  //         new anchor.BN(1_000_000) // Min 1M tokens
  //       )
  //       .signers([test_user_2])
  //       .accounts({
  //         buyer: test_user_2.publicKey,
  //         creator: test_user_1.publicKey, // Original token creator
  //         mint: newMint.publicKey, // Use the newly created mint
  //         platformFeeAccount: admin_wallet.publicKey
  //       })
  //       .rpc();

  //     console.log("Tokens bought:", buyTx);
  //   });

  it("should analyze bonding curve state before sales 1", async () => {
    console.log("\n🏁 INITIAL STATE (After Creation):");
    let state1 = await fetchBondingCurveState(
      new anchor.web3.PublicKey("AAruD8GshmPCQjqhVjSA4uZ6McZFpvyGh6j7GmubLggR")
    );
  });

  it("should buy token from bonding curve", async () => {
    // Add your test here.

    try {
      const tx = await program.methods
        .buyTokensBondingCurve(
          new anchor.BN(1_000_000_000),
          new anchor.BN(10_000_000)
        )
        .signers([test_user_1])
        .accounts({
          buyer: test_user_1.publicKey,
          creator: test_user_1.publicKey,
          mint: new anchor.web3.PublicKey(
            "AAruD8GshmPCQjqhVjSA4uZ6McZFpvyGh6j7GmubLggR"
          ),
          platformFeeAccount: test_user_2.publicKey
        })
        .rpc();

      console.log("Your transaction signature", tx);
    } catch (error) {
      console.error("Full error:", error);
      if (error.logs) {
        console.error("Transaction logs:", error.logs);
        throw error;
      }
    }
  });

  it("should analyze bonding curve state before sales 2", async () => {
    console.log("\n🏁 INITIAL STATE (After Creation):");
    let state1 = await fetchBondingCurveState(
      new anchor.web3.PublicKey("AAruD8GshmPCQjqhVjSA4uZ6McZFpvyGh6j7GmubLggR")
    );
  });

  it("should buy token from bonding curve", async () => {
    // Add your test here.

    try {
      const tx = await program.methods
        .buyTokensBondingCurve(
          new anchor.BN(1_000_000_000),
          new anchor.BN(10_000_000)
        )
        .signers([test_user_1])
        .accounts({
          buyer: test_user_1.publicKey,
          creator: test_user_1.publicKey,
          mint: new anchor.web3.PublicKey(
            "AAruD8GshmPCQjqhVjSA4uZ6McZFpvyGh6j7GmubLggR"
          ),
          platformFeeAccount: test_user_2.publicKey
        })
        .rpc();

      console.log("Your transaction signature", tx);
    } catch (error) {
      console.error("Full error:", error);
      if (error.logs) {
        console.error("Transaction logs:", error.logs);
        throw error;
      }
    }
  });

  it("should analyze bonding curve state before sales 3", async () => {
    console.log("\n🏁 INITIAL STATE (After Creation):");
    let state1 = await fetchBondingCurveState(
      new anchor.web3.PublicKey("AAruD8GshmPCQjqhVjSA4uZ6McZFpvyGh6j7GmubLggR")
    );
  });
  it("should buy token from bonding curve", async () => {
    // Add your test here.

    try {
      const tx = await program.methods
        .buyTokensBondingCurve(
          new anchor.BN(1_000_000_000),
          new anchor.BN(10_000_000)
        )
        .signers([test_user_1])
        .accounts({
          buyer: test_user_1.publicKey,
          creator: test_user_1.publicKey,
          mint: new anchor.web3.PublicKey(
            "AAruD8GshmPCQjqhVjSA4uZ6McZFpvyGh6j7GmubLggR"
          ),
          platformFeeAccount: test_user_2.publicKey
        })
        .rpc();

      console.log("Your transaction signature", tx);
    } catch (error) {
      console.error("Full error:", error);
      if (error.logs) {
        console.error("Transaction logs:", error.logs);
        throw error;
      }
    }
  });
});
