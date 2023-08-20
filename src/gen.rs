use alloy_sol_types::sol;
use ethers::contract::abigen;

abigen!(SimpleAccountFactory, "src/abi/SimpleAccountFactory.json",);

abigen!(SimpleAccount, "src/abi/SimpleAccount.json",);

abigen!(EntryPoint, "src/abi/EntryPoint.json",);

sol! {function execute(address dest, uint256 value, bytes calldata func);}
