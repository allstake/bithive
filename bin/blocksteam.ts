const TESTNET_URL = "https://blockstream.info/testnet/api";
const MAINNET_URL = "https://blockstream.info/api";

export async function getTransaction(
  txid: string,
  network: "testnet" | "mainnet",
) {
  const url = network === "testnet" ? TESTNET_URL : MAINNET_URL;
  const res = await fetch(`${url}/tx/${txid}/hex`);
  return res.text();
}

export async function getTransactionProof(
  txid: string,
  network: "testnet" | "mainnet",
): Promise<{
  merkle: string[];
  pos: number;
}> {
  const url = network === "testnet" ? TESTNET_URL : MAINNET_URL;
  const res = await fetch(`${url}/tx/${txid}/merkle-proof`);
  return res.json();
}

export async function getTransactionStatus(
  txid: string,
  network: "testnet" | "mainnet",
): Promise<{
  block_height: number;
  block_hash: string;
  block_time: number;
}> {
  const url = network === "testnet" ? TESTNET_URL : MAINNET_URL;
  const res = await fetch(`${url}/tx/${txid}/status`);
  return res.json();
}
