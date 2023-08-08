use ethers::contract::abigen;

abigen!(SimpleAccountFactory, "src/abi/SimpleAccountFactory.json",);

abigen!(SimpleAccount, "src/abi/SimpleAccount.json",);

abigen!(EntryPoint, "src/abi/EntryPoint.json",);
