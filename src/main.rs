use anchor_client::{Client, Cluster};
use marinade_client_rs::marinade::{instructions::stake_reserve, rpc_marinade::RpcMarinade};
use solana_sdk::{compute_budget::ComputeBudgetInstruction, signature::read_keypair_file, signer::Signer};
use anchor_client::{solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSimulateTransactionConfig}, solana_sdk::signature::Keypair};
use anchor_lang::prelude::*;
use std::{ str::FromStr,rc::Rc};
use solana_sdk::transaction::Transaction;
use std::time::Duration;
use std::thread;
#[tokio::main]
async fn main(){
    let url="https://api.mainnet-beta.solana.com";
    //let our_validator=Pubkey::from_str("sShosKd6uA5c1ZpVMxdsE6do13TLRWSMYsXbSMmNC77").unwrap();
    let our_validator=Pubkey::from_str("NipDoZC37aMQvv2fFq2moUyyiApQxszc7X4mvWHP2pZ").unwrap();
    let mar_prog = Pubkey::from_str("MarBmsSgKXdrN1egZf5sqe1TMai9K1rChYNDJgjq7aD").unwrap();
    let state_pubkey= Pubkey::from_str("8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC").unwrap();
    let kp= read_keypair_file("/Users/dev77/Desktop/solana-mainnet/mywallet.json").expect("KEYPAIR NOT FOUND");
    let kp2=read_keypair_file("/Users/dev77/Desktop/solana-mainnet/mywallet.json").expect("KEYPAIR NOT FOUND");
    let client=Client::new(
        Cluster::from_str(&url).unwrap(),
        Rc::new(kp),
        );
    let rpc_marinade_client = marinade_client_rs::marinade::rpc_marinade::RpcMarinade::new(
        &client, 
        mar_prog, 
        state_pubkey
    );
    let rpc_marinade_client= rpc_marinade_client.expect("cannot connect to rpc");
    let (val, count)=rpc_marinade_client.validator_list().expect("failed to get the validator validator_list");
    println!("{count}indexes in total");
    for  (pos,validators) in val.iter().enumerate(){
        if validators.validator_account==our_validator{
            //not dependent on my validator
            let reserve_balance = rpc_marinade_client.client.get_account(&Pubkey::from_str("Du3Ysj1wKbxPKkuPPnvzQLQh8oMSVifs3jGZjJWXFmHN").unwrap()).unwrap().lamports;
            let stake_delta = rpc_marinade_client.state.stake_delta(reserve_balance);
            let total_active_balance = rpc_marinade_client.state.validator_system.total_active_balance;
            let total_stake_delta = u64::try_from(stake_delta).expect("Stake delta overflow");
            let total_stake_target = total_active_balance.saturating_add(total_stake_delta);
            let validator_stake_target = rpc_marinade_client.state.validator_system.validator_stake_target(validators, total_stake_target).unwrap() / 1000000000;
            println!("{:?} active balance with position {pos} and stake target {validator_stake_target}",validators.active_balance/1000000000);
            let validator_active_balance=validators.active_balance/1000000000;
            if validator_active_balance>=validator_stake_target{
                println!("already reached staked target");
                break;
            }
            //create transaction
            let stake_account = Keypair::new();
            let program = client.program(mar_prog);
            let ix = stake_reserve(
                &program, 
                &state_pubkey, 
                &rpc_marinade_client.state, 
                pos as u32, 
                &our_validator, 
                &stake_account.pubkey(), 
                &kp2.pubkey()
            )
            .unwrap()
            .instruction(ComputeBudgetInstruction::set_compute_unit_price(100))
            .instruction(ComputeBudgetInstruction::set_compute_unit_limit(100000))
            .signer(&kp2)
            .signer(&stake_account)
            .instructions()
            .unwrap();
            let rpc_client = RpcClient::new(url.to_string());

            // simulating transaction
            
            'timing:loop{
                let latest_blockhash = rpc_client.get_latest_blockhash().await;
                let tx = Transaction::new_signed_with_payer(
                    ix.as_slice(),
                    Some(&kp2.pubkey()),
                    &[&kp2, &stake_account],
                    latest_blockhash.unwrap(),
                );
                let result = rpc_client.simulate_transaction_with_config(&tx, RpcSimulateTransactionConfig{sig_verify: true, ..RpcSimulateTransactionConfig::default()}).await;
                match result {
                    Ok(x) => {
                        println!("{x:#?}");
                        match x.value.logs{
                            Some(y) => {
                                let current= y[10].strip_prefix("Program log: Left:").unwrap();
                                let current= current.trim();
                                let current:i32= current.parse().unwrap();
                                let target=y[11].strip_prefix("Program log: Right:").unwrap();
                                let target=target.trim();
                                let target:i32 = target.parse().unwrap();
                                let slots_remaining=target-current;
                                let eta=slots_remaining/(4*60*60);// assuming 4 slots make up 1 second
                                if slots_remaining > 0 {
                                    println!("slots remaining are {slots_remaining} eta: {eta} hours");
                                    if (eta*60) > 10{// thresh hold 10 mins
                                      thread::sleep(Duration::from_secs(slots_remaining as u64/(4*60)));  
                                  }else{
                                    thread::sleep(Duration::from_secs(30));
                                  } 
                                    // ideally for many people this might not be required you can use tokio
                                }else{
                                    break 'timing;
                                }
                                
                            },
                            None =>println!("cannot extract logs"),
                        }
                        
                    },
                    Err(err) => eprintln!("Error: {}", err),
                }
            }
            let latest_blockhash = rpc_client.get_latest_blockhash().await;
            let tx = Transaction::new_signed_with_payer(
                ix.as_slice(),
                Some(&kp2.pubkey()),
                &[&kp2, &stake_account],
                latest_blockhash.unwrap(),
            );
            let result = rpc_client.send_and_confirm_transaction_with_spinner(&tx).await;
            match result {
                Ok(_) => println!("Transaction signature: {:?}", result),
                Err(err) => eprintln!("Error: {}", err),
            }
            break;
        }
        
    }

}
