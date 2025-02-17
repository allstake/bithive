const TESTNET3_URL = "https://mempool.space/testnet4/api";
const TESTNET4_URL = "https://mempool.space/testnet4/api";
const SIGNET_URL = "https://mempool.space/signet/api";
const MAINNET_URL = "https://mempool.space/api";

type Network = "signet" | "testnet3" | "testnet4" | "mainnet";

function getUrl(network: Network) {
  if (network === "signet") return SIGNET_URL;
  if (network === "testnet3") return TESTNET3_URL;
  if (network === "testnet4") return TESTNET4_URL;
  return MAINNET_URL;
}

export async function getTransaction(txid: string, network: Network) {
  const url = getUrl(network);
  const res = await fetch(`${url}/tx/${txid}/hex`);
  return res.text();
}

export async function broadcastTransaction(txHex: string, network: Network) {
  const url = getUrl(network);
  const res = await fetch(`${url}/tx`, { method: "POST", body: txHex });
  return res.text();
}
