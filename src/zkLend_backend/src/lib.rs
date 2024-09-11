use ic_cdk::export::{
    candid::{CandidType, Deserialize},
    Principal,
};
use std::collections::HashMap;
use ic_cdk_macros::*;
use ic_cdk::api::call::CallResult;

#[derive(CandidType, Deserialize, Clone)]
struct User {
    balance: u64,
    staked_amount: u64,
    borrowed_amount: u64,
}

#[derive(CandidType, Deserialize)]
struct CanisterState {
    users: HashMap<Principal, User>,
    total_staked: u64,
    total_borrowed: u64,
}

thread_local! {
    static STATE: ic_cdk::storage::RefCell<CanisterState> = ic_cdk::storage::RefCell::new(CanisterState {
        users: HashMap::new(),
        total_staked: 0,
        total_borrowed: 0,
    });
}


use ic_cdk::api::management_canister::main::raw_rand;


const TOKEN_CANISTER_ID: &str = "ryjl3-tyaaa-aaaaa-aaaba-cai";

#[update]
async fn deposit(amount: u64) -> Result<(), String> {
    let caller = ic_cdk::caller();
    
    // Call the token canister to transfer tokens
    let transfer_result: CallResult<(Result<u64, String>,)> = ic_cdk::call(
        Principal::from_text(TOKEN_CANISTER_ID).map_err(|e| format!("Invalid canister ID: {}", e))?,
        "icrc1_transfer",
        (TransferArgs {
            from_subaccount: None,
            to: Account {
                owner: ic_cdk::id(),
                subaccount: None,
            },
            amount: amount,
            fee: None,
            memo: None,
            created_at_time: None,
        },),
    )
    .await;

    match transfer_result {
        Ok((Ok(_),)) => {
            // If transfer successful, update the user's balance
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                let user = state.users.entry(caller).or_insert(User {
                    balance: 0,
                    staked_amount: 0,
                    borrowed_amount: 0,
                });
                user.balance += amount;
            });
            Ok(())
        }
        Ok((Err(e),)) => Err(format!("Token transfer failed: {}", e)),
        Err(e) => Err(format!("Inter-canister call failed: {}", e)),
    }
}

// Add these structs for the ICRC-1 transfer
#[derive(CandidType, Deserialize)]
struct Account {
    owner: Principal,
    subaccount: Option<[u8; 32]>,
}

#[derive(CandidType, Deserialize)]
struct TransferArgs {
    from_subaccount: Option<[u8; 32]>,
    to: Account,
    amount: u64,
    fee: Option<u64>,
    memo: Option<Vec<u8>>,
    created_at_time: Option<u64>,
}

#[update]
async fn withdraw(amount: u64) -> Result<(), String> {
    let caller = ic_cdk::caller();
    
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let user = state.users.get_mut(&caller).ok_or("User not found")?;
        if user.balance < amount {
            return Err("Insufficient balance".to_string());
        }
        user.balance -= amount;
        Ok(())
    })?;

    // Transfer tokens from canister to user
    let transfer_result: CallResult<(Result<u64, String>,)> = ic_cdk::call(
        Principal::from_text(TOKEN_CANISTER_ID).map_err(|e| format!("Invalid canister ID: {}", e))?,
        "icrc1_transfer",
        (TransferArgs {
            from_subaccount: None,
            to: Account {
                owner: caller,
                subaccount: None,
            },
            amount: amount,
            fee: None,
            memo: None,
            created_at_time: None,
        },),
    )
    .await;

    match transfer_result {
        Ok((Ok(_),)) => Ok(()),
        Ok((Err(e),)) => {
            // Revert the balance change if transfer fails
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                let user = state.users.get_mut(&caller).unwrap();
                user.balance += amount;
            });
            Err(format!("Token transfer failed: {}", e))
        },
        Err(e) => {
            // Revert the balance change if inter-canister call fails
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                let user = state.users.get_mut(&caller).unwrap();
                user.balance += amount;
            });
            Err(format!("Inter-canister call failed: {}", e))
        },
    }
}

#[update]
async fn stake(amount: u64) -> Result<(), String> {
    let caller = ic_cdk::caller();
    
    // Transfer tokens from user to canister
    let transfer_result: CallResult<(Result<u64, String>,)> = ic_cdk::call(
        Principal::from_text(TOKEN_CANISTER_ID).map_err(|e| format!("Invalid canister ID: {}", e))?,
        "icrc1_transfer",
        (TransferArgs {
            from_subaccount: None,
            to: Account {
                owner: ic_cdk::id(),
                subaccount: None,
            },
            amount: amount,
            fee: None,
            memo: None,
            created_at_time: None,
        },),
    )
    .await;

    match transfer_result {
        Ok((Ok(_),)) => {
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                let user = state.users.entry(caller).or_insert(User {
                    balance: 0,
                    staked_amount: 0,
                    borrowed_amount: 0,
                });
                user.staked_amount += amount;
                state.total_staked += amount;
            });
            Ok(())
        },
        Ok((Err(e),)) => Err(format!("Token transfer failed: {}", e)),
        Err(e) => Err(format!("Inter-canister call failed: {}", e)),
    }
}

#[update]
async fn unstake(amount: u64) -> Result<(), String> {
    let caller = ic_cdk::caller();
    
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let user = state.users.get_mut(&caller).ok_or("User not found")?;
        if user.staked_amount < amount {
            return Err("Insufficient staked amount".to_string());
        }
        user.staked_amount -= amount;
        state.total_staked -= amount;
        Ok(())
    })?;

    // Transfer tokens from canister to user
    let transfer_result: CallResult<(Result<u64, String>,)> = ic_cdk::call(
        Principal::from_text(TOKEN_CANISTER_ID).map_err(|e| format!("Invalid canister ID: {}", e))?,
        "icrc1_transfer",
        (TransferArgs {
            from_subaccount: None,
            to: Account {
                owner: caller,
                subaccount: None,
            },
            amount: amount,
            fee: None,
            memo: None,
            created_at_time: None,
        },),
    )
    .await;

    match transfer_result {
        Ok((Ok(_),)) => Ok(()),
        Ok((Err(e),)) => {
            // Revert the staked amount change if transfer fails
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                let user = state.users.get_mut(&caller).unwrap();
                user.staked_amount += amount;
                state.total_staked += amount;
            });
            Err(format!("Token transfer failed: {}", e))
        },
        Err(e) => {
            // Revert the staked amount change if inter-canister call fails
            STATE.with(|state| {
                let mut state = state.borrow_mut();
                let user = state.users.get_mut(&caller).unwrap();
                user.staked_amount += amount;
                state.total_staked += amount;
            });
            Err(format!("Inter-canister call failed: {}", e))
        },
    }
}

#[update]
fn borrow(amount: u64) -> Result<(), String> {
    let caller = ic_cdk::caller();
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let user = state.users.get_mut(&caller).ok_or("User not found")?;
        let max_borrow = user.staked_amount / 2; // Allow borrowing up to 50% of staked amount
        if amount > max_borrow {
            return Err("Borrow amount exceeds allowed limit".to_string());
        }
        user.borrowed_amount += amount;
        user.balance += amount;
        state.total_borrowed += amount;
        Ok(())
    })
}

#[update]
fn repay(amount: u64) -> Result<(), String> {
    let caller = ic_cdk::caller();
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let user = state.users.get_mut(&caller).ok_or("User not found")?;
        if user.balance < amount || user.borrowed_amount < amount {
            return Err("Insufficient balance or borrowed amount".to_string());
        }
        user.borrowed_amount -= amount;
        user.balance -= amount;
        state.total_borrowed -= amount;
        Ok(())
    })
}

#[query]
fn get_user_info(user: Principal) -> User {
    STATE.with(|state| {
        state.borrow().users.get(&user).cloned().unwrap_or(User {
            balance: 0,
            staked_amount: 0,
            borrowed_amount: 0,
        })
    })
}

#[query]
fn get_canister_info() -> (u64, u64) {
    STATE.with(|state| {
        let state = state.borrow();
        (state.total_staked, state.total_borrowed)
    })
}

// Keep the original greet function
#[ic_cdk::query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}
