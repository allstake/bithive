#!node_modules/.bin/ts-node

import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import {
  deployBip322Verifier,
  deployBitHive,
  deployBtcLightClient,
} from "./commands/deploy";
import { init, initBip322 } from "./commands/init";
import { submitDeposit } from "./commands/submit_deposit";
import { queueWithdrawal } from "./commands/queue_withdrawal";
import { submitWithdrawal } from "./commands/submit_withdrawal";
import { signWithdrawal } from "./commands/sign_withdrawal";
import { upgrade } from "./commands/upgrade";
import { soloWithdraw } from "./commands/solo_withdraw";

yargs(hideBin(process.argv))
  .strict()
  .help()
  .command(deployBitHive)
  .command(deployBtcLightClient)
  .command(deployBip322Verifier)
  .command(submitDeposit)
  .command(queueWithdrawal)
  .command(submitWithdrawal)
  .command(signWithdrawal)
  .command(init)
  .command(initBip322)
  .command(upgrade)
  .command(soloWithdraw)
  .parse();
