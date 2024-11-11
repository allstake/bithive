#!node_modules/.bin/ts-node

import yargs from "yargs";
import { hideBin } from "yargs/helpers";
import {
  deployBip322Verifier,
  deployBtcClient,
  deployBtcLightClient,
} from "./commands/deploy";
import { init, initBip322 } from "./commands/init";
import { submitDeposit } from "./commands/submit_deposit";
import { queueWithdraw } from "./commands/queue_withdraw";
import { submitWithdraw } from "./commands/submit_withdraw";
import { signWithdraw } from "./commands/sign_withdraw";
import { upgrade } from "./commands/upgrade";

yargs(hideBin(process.argv))
  .strict()
  .help()
  .command(deployBtcClient)
  .command(deployBtcLightClient)
  .command(deployBip322Verifier)
  .command(submitDeposit)
  .command(queueWithdraw)
  .command(submitWithdraw)
  .command(signWithdraw)
  .command(init)
  .command(initBip322)
  .command(upgrade)
  .parse();
