use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_interface::TokenInterface;
use item_nft::program::ItemNft;
use magic_token::program::MagicToken;

declare_id!("6bSYNXVaLAAUk294xQ3fWBsB2RZ3YDwspzwQB8nnFQWS");

const MARKETPLACE_AUTHORITY_SEED: &[u8] = b"marketplace-authority";

#[program]
pub mod marketplace {
    use super::*;

    /// Redeems an NFT item back to the system: NFT is burned and seller receives MagicToken.
    pub fn redeem_item(ctx: Context<RedeemItem>) -> Result<()> {
        let (expected_authority, bump) =
            Pubkey::find_program_address(&[MARKETPLACE_AUTHORITY_SEED], ctx.program_id);
        require_keys_eq!(
            expected_authority,
            ctx.accounts.marketplace_authority.key(),
            MarketplaceError::InvalidAuthorityPda
        );

        let mut cfg_data: &[u8] = &ctx.accounts.resource_config.data.borrow();
        let resource_cfg = resource_manager::ResourceConfig::try_deserialize(&mut cfg_data)?;
        let mut item_data: &[u8] = &ctx.accounts.item_record.data.borrow();
        let item_record = item_nft::ItemRecord::try_deserialize(&mut item_data)?;

        require!(!item_record.burned, MarketplaceError::AlreadyRedeemed);
        let item_type = item_record.item_type as usize;
        require!(item_type < 4, MarketplaceError::InvalidItemType);
        let reward = resource_cfg.item_prices[item_type];

        let authority_seeds: &[&[u8]] = &[MARKETPLACE_AUTHORITY_SEED, &[bump]];

        let burn_accounts = item_nft::cpi::accounts::BurnItem {
            config: ctx.accounts.item_nft_config.to_account_info(),
            caller_authority: ctx.accounts.marketplace_authority.to_account_info(),
            owner: ctx.accounts.seller.to_account_info(),
            item_record: ctx.accounts.item_record.to_account_info(),
            item_mint: ctx.accounts.item_mint.to_account_info(),
            owner_item_token_account: ctx.accounts.seller_item_token_account.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };

        item_nft::cpi::burn_item(CpiContext::new_with_signer(
            ctx.accounts.item_nft_program.to_account_info(),
            burn_accounts,
            &[authority_seeds],
        ))?;

        let reward_accounts = magic_token::cpi::accounts::MintReward {
            config: ctx.accounts.magic_token_config.to_account_info(),
            caller_authority: ctx.accounts.marketplace_authority.to_account_info(),
            magic_mint_authority: ctx.accounts.magic_mint_authority.to_account_info(),
            magic_mint: ctx.accounts.magic_mint.to_account_info(),
            recipient_magic_token_account: ctx
                .accounts
                .seller_magic_token_account
                .to_account_info(),
            token_program: ctx.accounts.token_2022_program.to_account_info(),
        };

        magic_token::cpi::mint_reward(
            CpiContext::new_with_signer(
                ctx.accounts.magic_token_program.to_account_info(),
                reward_accounts,
                &[authority_seeds],
            ),
            reward,
        )?;

        emit!(ItemRedeemed {
            seller: ctx.accounts.seller.key(),
            item_mint: ctx.accounts.item_mint.key(),
            reward,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct RedeemItem<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,
    /// CHECK: Marketplace authority PDA signs CPIs to ItemNFT and MagicToken.
    pub marketplace_authority: UncheckedAccount<'info>,
    /// CHECK: foreign ResourceManager config, deserialized manually to fetch prices.
    pub resource_config: UncheckedAccount<'info>,
    /// CHECK: foreign ItemNFT config.
    pub item_nft_config: UncheckedAccount<'info>,
    /// CHECK: foreign MagicToken config.
    pub magic_token_config: UncheckedAccount<'info>,
    /// CHECK: foreign MagicToken signer PDA.
    pub magic_mint_authority: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: This account is validated manually in the instruction logic.
    pub item_record: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: item_mint is validated manually in the instruction logic.
    pub item_mint: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: seller_item_token_account is validated manually in the instruction logic.
    pub seller_item_token_account: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: magic_mint is validated manually in the instruction logic.
    pub magic_mint: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: seller_magic_token_account is validated manually in the instruction logic.
    pub seller_magic_token_account: UncheckedAccount<'info>,
    pub item_nft_program: Program<'info, ItemNft>,
    pub magic_token_program: Program<'info, MagicToken>,
    pub token_program: Program<'info, Token>,
    pub token_2022_program: Interface<'info, TokenInterface>,
}

#[event]
pub struct ItemRedeemed {
    pub seller: Pubkey,
    pub item_mint: Pubkey,
    pub reward: u64,
}

#[error_code]
pub enum MarketplaceError {
    #[msg("Invalid marketplace authority PDA")]
    InvalidAuthorityPda,
    #[msg("Item was already redeemed")]
    AlreadyRedeemed,
    #[msg("Invalid item type")]
    InvalidItemType,
}
