import { CommandModule } from "yargs";
import { getConfig } from "../config";
import { envBuilder, nearTGas } from "../helper";
import { initNear } from "../near";

interface Args {
  env: string;
  tx: string;
}

export const submitWithdraw: CommandModule<unknown, Args> = {
  command: "withdraw",
  describe: "Submit a BTC withdraw transaction",
  builder: {
    env: envBuilder,
    tx: {
      type: "string",
      describe: "Hex encoded withdraw transaction",
      demandOption: true,
    },
  },
  async handler({ env, tx }) {
    const config = await getConfig(env);
    const { signer } = await initNear(env);

    const args = {
      tx_hex: tx,
      user_pubkey_hex:
        "0299b4097603b073aa2390203303fe0e60c87bd2af8e621a3df22818c40e3dd217",
      deposit_vin: 0,
      tx_block_hash:
        "000000000000000000009b7b2113ecfda194a82a511cf40453e7ec7f93d2779f",
      tx_index: 554,
      merkle_proof: [
        "edc43635a5dad5c2158ebc969d9e76f7d871fd930f120cd176a8cca90bf8c72c",
        "53559f59917fd1d9ee52720a8aa39e64c1c04738bd49d44cfc6cf3da3fc2f065",
        "06683636e725103d4203961b4eab5f6db5cd7b4828be2d6d0cc2affc96cc7661",
        "1be1ca713868f37aa7968f97e032a5594d5fa16336f26889e3a3a0c35dfcd557",
        "1791e141e20b68fc8c01094643869fe4e60d8f25cc3a69bdb9c325e49b2f83f3",
        "9d86aa3b5d1a969bb7fbc2aa3c73d5d9e55ed9cadbb5d80f7fc0205c3424a765",
        "43ba8907658c3f4cce10f3bca2838ebd73363c97057b37b38c96a4585e864465",
        "d112d56c239d6a184861c0ea510f8fd1970b94f3891e06fd41544311af61e250",
        "0a2918a7c0ef370034fb782fbb8294f655d22b35ee2d16ab3019fd49e4f0cef4",
        "f9b5c77c4dc7f5d733288e9528e68996165a41c14e1fc0cf33ddac1fc4c52c9d",
        "7ae32f95f3d7a5f32b288fb278bc383b1e38f0d0a447327f2ac128925ca701fb",
        "d277683feacdc692d84090ebd89c412a85773b240932c9b98c935ebf2d761b70",
      ],
    };

    await signer.functionCall({
      contractId: config.accountIds.btcClient,
      methodName: "submit_deposit_tx",
      args: args,
      gas: nearTGas(100),
    });

    console.log("Submitted withdraw tx");
  },
};
