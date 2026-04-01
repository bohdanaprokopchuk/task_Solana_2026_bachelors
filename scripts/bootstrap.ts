import * as anchor from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import { TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";

async function main() {
  const argv = await yargs(hideBin(process.argv))
    .option("cluster", { type: "string", default: "localnet" })
    .parse();

  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const resourceManager = anchor.workspace.ResourceManager as anchor.Program<any>;
  const itemNft = anchor.workspace.ItemNft as anchor.Program<any>;
  const magicToken = anchor.workspace.MagicToken as anchor.Program<any>;
  const search = anchor.workspace.Search as anchor.Program<any>;
  const crafting = anchor.workspace.Crafting as anchor.Program<any>;
  const marketplace = anchor.workspace.Marketplace as anchor.Program<any>;

  const [resourceConfig] = PublicKey.findProgramAddressSync([Buffer.from("resource-config")], resourceManager.programId);
  const [itemConfig] = PublicKey.findProgramAddressSync([Buffer.from("item-config")], itemNft.programId);
  const [magicConfig] = PublicKey.findProgramAddressSync([Buffer.from("magic-config")], magicToken.programId);
  const [resourceManagerAuthority] = PublicKey.findProgramAddressSync([Buffer.from("resource-manager-authority")], resourceManager.programId);
  const [searchAuthority] = PublicKey.findProgramAddressSync([Buffer.from("search-authority")], search.programId);
  const [craftingAuthority] = PublicKey.findProgramAddressSync([Buffer.from("crafting-authority")], crafting.programId);
  const [marketplaceAuthority] = PublicKey.findProgramAddressSync([Buffer.from("marketplace-authority")], marketplace.programId);
  const [magicMintAuthority] = PublicKey.findProgramAddressSync([Buffer.from("magic-mint-authority")], magicToken.programId);

  const resourceMints = Array.from({ length: 6 }, () => Keypair.generate());
  const magicMint = Keypair.generate();

  const itemPrices = [10, 15, 25, 40];

  console.log(`Bootstrapping cluster: ${argv.cluster}`);

  await resourceManager.methods
    .initializeConfig(searchAuthority, craftingAuthority, itemPrices)
    .accounts({
      admin: provider.wallet.publicKey,
      config: resourceConfig,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  await itemNft.methods
    .initializeConfig(craftingAuthority, marketplaceAuthority)
    .accounts({
      admin: provider.wallet.publicKey,
      config: itemConfig,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  await magicToken.methods
    .initializeConfig(marketplaceAuthority)
    .accounts({
      admin: provider.wallet.publicKey,
      config: magicConfig,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  const resourceNames = [
    ["Wood", "WOOD"],
    ["Iron", "IRON"],
    ["Gold", "GOLD"],
    ["Leather", "LEATHER"],
    ["Stone", "STONE"],
    ["Diamond", "DIAMOND"],
  ];

  for (const [idx, mint] of resourceMints.entries()) {
    const [name, symbol] = resourceNames[idx];
    await resourceManager.methods
      .initResourceMint(idx, name, symbol, `https://example.com/resource/${idx}.json`)
      .accounts({
        admin: provider.wallet.publicKey,
        config: resourceConfig,
        resourceMint: mint.publicKey,
        resourceManagerAuthority,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([mint])
      .rpc();

    console.log(`Resource mint initialized: ${symbol} -> ${mint.publicKey.toBase58()}`);
  }

  await magicToken.methods
    .initializeMint("MagicToken", "MAGIC", "https://example.com/magic.json")
    .accounts({
      admin: provider.wallet.publicKey,
      config: magicConfig,
      magicMint: magicMint.publicKey,
      magicMintAuthority,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .signers([magicMint])
    .rpc();

  console.log(`MagicToken mint initialized: ${magicMint.publicKey.toBase58()}`);
  console.log("Bootstrap complete");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
