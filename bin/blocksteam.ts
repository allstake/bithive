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
