const TESTNET3_URL = "https://mempool.space/testnet4/api";
const TESTNET4_URL = "https://mempool.space/testnet4/api";
const MAINNET_URL = "https://mempool.space/api";

type Network = "testnet3" | "testnet4" | "mainnet";

function getUrl(network: Network) {
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
