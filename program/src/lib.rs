use borsh::{ BorshDeserialize, BorshSerialize };
use solana_program::{
    log::sol_log_compute_units,
    account_info::{ next_account_info, AccountInfo },
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use std::io::ErrorKind::InvalidData;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, TokenAccount, Transfer, MintTo};
use anchor_lang::solana_program::program_option::COption;
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct ChatMessage {
    pub archive_id: String,
    pub created_on: String
}

// example arweave tx (length 43)
// 1seRanklLU_1VTGkEk7P0xAwMJfA7owA1JHW5KyZKlY
// ReUohI9tEmXQ6EN9H9IkRjY9bSdgql_OdLUCOeMEte0
const DUMMY_TX_ID: &str = "0000000000000000000000000000000000000000000";
const DUMMY_CREATED_ON: &str = "0000000000000000"; // milliseconds, 16 digits
pub fn get_init_chat_message() -> ChatMessage {
    ChatMessage{ archive_id: String::from(DUMMY_TX_ID), created_on: String::from(DUMMY_CREATED_ON) }
}
pub fn get_init_chat_messages() -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    for _ in 0..20 {
        messages.push(get_init_chat_message());
    }
    return messages;
}

entrypoint!(process_instruction);


pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8]
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let account = next_account_info(accounts_iter)?;
    if account.owner != program_id {
        msg!("This account {} is not owned by this program {} and cannot be updated!", account.key, program_id);
    }

    sol_log_compute_units();

    let instruction_data_message = ChatMessage::try_from_slice(instruction_data).map_err(|err| {
        msg!("Attempt to deserialize instruction data has failed. {:?}", err);
        ProgramError::InvalidInstructionData
    })?;
    msg!("Instruction_data message object {:?}", instruction_data_message);

    let mut existing_data_messages = match <Vec<ChatMessage>>::try_from_slice(&account.data.borrow_mut()) {
        Ok(data) => data,
        Err(err) => {
            if err.kind() == InvalidData {
                msg!("InvalidData so initializing account data");
                get_init_chat_messages()
            } else {
                panic!("Unknown error decoding account data {:?}", err)
            }
        }
    };
    let index = existing_data_messages.iter().position(|p| p.archive_id == String::from(DUMMY_TX_ID)).unwrap(); // find first dummy data entry
    msg!("Found index {}", index);
    existing_data_messages[index] = instruction_data_message; // set dummy data to new entry
    let updated_data = existing_data_messages.try_to_vec().expect("Failed to encode data."); // set messages object back to vector data
    msg!("Final existing_data_messages[index] {:?}", existing_data_messages[index]);

    // data algorithm for storing data into account and then archiving into Arweave
    // 1. Each ChatMessage object will be prepopulated for txt field having 43 characters (length of a arweave tx).
    // Each ChatMessageContainer will be prepopulated with 10 ChatMessage objects with dummy data.
    // 2. Client will submit an arweave tx for each message; get back the tx id; and submit it to our program.
    // 3. This tx id will be saved to the Solana program and be used for querying back to arweave to get actual data.
    let data = &mut &mut account.data.borrow_mut();
    msg!("Attempting save data.");
    data[..updated_data.len()].copy_from_slice(&updated_data);    
    let saved_data = <Vec<ChatMessage>>::try_from_slice(data)?;
    msg!("ChatMessage has been saved to account data. {:?}", saved_data[index]);
    sol_log_compute_units();

    msg!("End program.");
    Ok(())
}




#[program]
pub mod dog_money {
    use super::*;
    pub fn initialize_user(ctx: Context<InitializeUser>, amount: u64, nonce: u8) -> ProgramResult {
        let user_data = &mut ctx.accounts.user_data;
        user_data.first_deposit = ctx.accounts.clock.unix_timestamp;

        // Transfer USDC from user to vault
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_usdc.to_account_info(),
            to: ctx.accounts.program_vault.to_account_info(),
            authority: ctx.accounts.authority.clone(),
        };
        let cpi_program = ctx.accounts.token_program.clone();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        // Mint 1,0000x dog money to user account
        let dog_money_amount = amount.checked_mul(1000).unwrap();
        let seeds = &[ctx.accounts.usdc_mint.to_account_info().key.as_ref(), &[nonce], ];
        let signer = &[&seeds[..]];
        let cpi_accounts = MintTo {
            mint: ctx.accounts.dog_money_mint.to_account_info(),
            to: ctx.accounts.user_dog_money.to_account_info(),
            authority: ctx.accounts.program_signer.clone()
        };
        let cpi_program = ctx.accounts.token_program.clone();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::mint_to(cpi_ctx, dog_money_amount)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeUser<'info> {
    program_signer: AccountInfo<'info>,
    #[account(associated = authority, with = usdc_mint)]
    user_data: ProgramAccount<'info, UserData>,
    #[account(signer)]
    authority: AccountInfo<'info>,
    usdc_mint: CpiAccount<'info, Mint>,
    #[account(mut, "user_usdc.owner == *authority.key")]
    user_usdc: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    program_vault: CpiAccount<'info, TokenAccount>,
    #[account(mut,
    "dog_money_mint.mint_authority == COption::Some(*program_signer.key)")]
    dog_money_mint: CpiAccount<'info, Mint>,
    #[account(mut, "user_dog_money.owner == *authority.key")]
    user_dog_money: CpiAccount<'info, TokenAccount>,
    // We already know its address and that it's executable
    #[account(executable, "token_program.key == &token::ID")]
    token_program: AccountInfo<'info>,
    rent: Sysvar<'info, Rent>,
    system_program: AccountInfo<'info>,
    clock: Sysvar<'info, Clock>,
}


#[associated]
pub struct UserData {
    pub first_deposit: i64,
}

struct Decimal {
    pub value: u128,
    pub decimals: u32,
}

/// Define the type of state stored in accounts
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct PriceFeedAccount {
    /// number of greetings
    pub answer: u128,
}

impl Decimal {
    pub fn new(value: u128, decimals: u32) -> Self {
        Decimal { value, decimals }
    }
}

impl std::fmt::Display for Decimal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut scaled_val = self.value.to_string();
        if scaled_val.len() <= self.decimals as usize {
            scaled_val.insert_str(
                0,
                &vec!["0"; self.decimals as usize - scaled_val.len()].join(""),
            );
            scaled_val.insert_str(0, "0.");
        } else {
            scaled_val.insert(scaled_val.len() - self.decimals as usize, '.');
        }
        f.write_str(&scaled_val)
    }
}

// Declare and export the program's entrypoint
entrypoint!(get_price);

// Program entrypoint's implementation
pub fn get_price(
    _program_id: &Pubkey, // Ignored
    accounts: &[AccountInfo], // Public key of the account to read price data from
    _instruction_data: &[u8], // Ignored
) -> ProgramResult {
    msg!("Chainlink Solana Demo program entrypoint");

    let accounts_iter = &mut accounts.iter();
    // This is the account of our our account
    let my_account = next_account_info(accounts_iter)?;
    // This is the account of the price feed data
    let feed_account = next_account_info(accounts_iter)?;

    const DECIMALS: u32 = 9;

    let price = chainlink::get_price(&chainlink::id(), feed_account)?;

    if let Some(price) = price {
        let decimal = Decimal::new(price, DECIMALS);
        msg!("Price is {}", decimal);
    } else {
        msg!("No current price");
    }

     // Store the price ourselves
     let mut price_data_account = PriceFeedAccount::try_from_slice(&my_account.data.borrow())?;
     price_data_account.answer = price.unwrap_or(0);
     price_data_account.serialize(&mut &mut my_account.data.borrow_mut()[..])?;


    Ok(())
}


#[cfg(test)]
mod test {
    use {
        super::*,
        assert_matches::*,
        solana_program::instruction::{AccountMeta, Instruction},
        solana_program_test::*,
        solana_sdk::{signature::Signer, transaction::Transaction},
    };

    #[tokio::test]
    async fn test_transaction() {
        let program_id = Pubkey::new_unique();

        let (mut banks_client, payer, recent_blockhash) = ProgramTest::new(
            "bpf_program_template",
            program_id,
            processor!(process_instruction),
        )
        .start()
        .await;

        let mut transaction = Transaction::new_with_payer(
            &[Instruction {
                program_id,
                accounts: vec![AccountMeta::new(payer.pubkey(), false)],
                data: vec![1, 2, 3],
            }],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[&payer], recent_blockhash);

        assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
    }
}


// Sanity tests
#[cfg(test)]
mod test {
    use super::*;
    use solana_program::clock::Epoch;
    //use std::mem;

    #[test]
    fn test_sanity() {
        let program_id = Pubkey::default();
        let key = Pubkey::default();
        let mut lamports = 0;
        let messages = get_init_chat_messages(); 
        let mut data = messages.try_to_vec().unwrap();
        let owner = Pubkey::default();
        let account = AccountInfo::new(
            &key,
            false,
            true,
            &mut lamports,
            &mut data,
            &owner,
            false,
            Epoch::default(),
        );
        
        let archive_id = "abcdefghijabcdefghijabcdefghijabcdefghijabc";
        let created_on = "0001621449453837";
        let instruction_data_chat_message = ChatMessage{ archive_id: String::from(archive_id), created_on: String::from(created_on) };
        let instruction_data = instruction_data_chat_message.try_to_vec().unwrap();

        let accounts = vec![account];

        process_instruction(&program_id, &accounts, &instruction_data).unwrap();
        let chat_messages = &<Vec<ChatMessage>>::try_from_slice(&accounts[0].data.borrow())
        .unwrap()[0];
        let test_archive_id = &chat_messages.archive_id;
        let test_created_on = &chat_messages.created_on;
        println!("chat message {:?}", &chat_messages);
        // I added first data and expect it to contain the given data
        assert_eq!(
            String::from(archive_id).eq(test_archive_id),
            true
        );
        assert_eq!(
            String::from(created_on).eq(test_created_on),
            true
        );
    }
}
