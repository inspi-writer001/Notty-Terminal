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

describe("notty-smart-contract", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace
    .NottySmartContract as Program<NottySmartContract>;

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

  //   it("should create bonding curve token", async () => {
  //     let new_mint = anchor.web3.Keypair.generate();

  //     try {
  //       const tx = await program.methods
  //         .createBondingCurveToken(
  //           "Hello Token",
  //           "HTK",
  //           "https://fastly.picsum.photos/id/237/200/300.jpg?hmac=TmmQSbShHz9CdQm0NkEjx1Dyh_Y984R9LpNrpvH2D_U"
  //         )
  //         .accounts({
  //           creator: test_user_1.publicKey,
  //           mintAccount: new_mint.publicKey
  //         })
  //         .signers([test_user_1, new_mint])
  //         .rpc();

  //       console.log("Your transaction signature", tx);
  //     } catch (error) {
  //       console.error("Full error:", error);
  //       if (error.logs) {
  //         console.error("Transaction logs:", error.logs);
  //       }
  //     }
  //   });

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

  //   it("should buy token from bonding curve", async () => {
  //     // Add your test here.

  //     try {
  //       const tx = await program.methods
  //         .buyTokensBondingCurve(new anchor.BN(1_000_000), new anchor.BN(10))
  //         .signers([test_user_1])
  //         .accounts({
  //           buyer: test_user_1.publicKey,
  //           creator: test_user_1.publicKey,
  //           mint: new anchor.web3.PublicKey(
  //             "9NCvTXAVWifeg3FdAPuhroa3HQBsDtAVYgw3xnPtAz6X"
  //           ),
  //           platformFeeAccount: test_user_2.publicKey
  //         })
  //         .rpc();

  //       console.log("Your transaction signature", tx);
  //     } catch (error) {
  //       console.error("Full error:", error);
  //       if (error.logs) {
  //         console.error("Transaction logs:", error.logs);
  //         throw error;
  //       }
  //     }
  //   });

  //   it("should buy token from bonding curve", async () => {
  //     // Add your test here.

  //     try {
  //       const tx = await program.methods
  //         .buyTokensBondingCurve(new anchor.BN(1_000_000_000), new anchor.BN(10))
  //         .signers([test_user_1])
  //         .accounts({
  //           buyer: test_user_1.publicKey,
  //           creator: test_user_1.publicKey,
  //           mint: new anchor.web3.PublicKey(
  //             "9NCvTXAVWifeg3FdAPuhroa3HQBsDtAVYgw3xnPtAz6X"
  //           ),
  //           platformFeeAccount: test_user_2.publicKey
  //         })
  //         .rpc();

  //       console.log("Your transaction signature", tx);
  //     } catch (error) {
  //       console.error("Full error:", error);
  //       if (error.logs) {
  //         console.error("Transaction logs:", error.logs);
  //         throw error;
  //       }
  //     }
  //   });
  //   it("should buy token from bonding curve", async () => {
  //     // Add your test here.

  //     try {
  //       const tx = await program.methods
  //         .buyTokensBondingCurve(new anchor.BN(1_000_000), new anchor.BN(10))
  //         .signers([test_user_1])
  //         .accounts({
  //           buyer: test_user_1.publicKey,
  //           creator: test_user_1.publicKey,
  //           mint: new anchor.web3.PublicKey(
  //             "9NCvTXAVWifeg3FdAPuhroa3HQBsDtAVYgw3xnPtAz6X"
  //           ),
  //           platformFeeAccount: test_user_2.publicKey
  //         })
  //         .rpc();

  //       console.log("Your transaction signature", tx);
  //     } catch (error) {
  //       console.error("Full error:", error);
  //       if (error.logs) {
  //         console.error("Transaction logs:", error.logs);
  //         throw error;
  //       }
  //     }
  //   });

  //   it("should create token and buy from bonding curve", async () => {
  //     // Step 1: Create a new bonding curve token
  //     const newMint = anchor.web3.Keypair.generate();

  //     const createTx = await program.methods
  //       .createBondingCurveToken("Test Token", "TEST", "https://example.com")
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
  //         new anchor.BN(1_000_000) // Min 1M tokens (adjust based on expected return)
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
});
