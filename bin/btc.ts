import * as bitcoin from "bitcoinjs-lib";

export function getWitnessUtxo(out: any): any {
  delete out.address;
  out.script = Buffer.from(out.script, "hex");
  return out;
}

export function depositScriptV1(
  userPubkey: Buffer,
  bithivePubkey: Buffer,
  sequence: number,
): Buffer {
  return bitcoin.script.fromASM(
    `
    OP_IF
        ${bitcoin.script.number.encode(sequence).toString("hex")}
        OP_CHECKSEQUENCEVERIFY
        OP_DROP
        ${userPubkey.toString("hex")}
        OP_CHECKSIG
    OP_ELSE
        OP_2
        ${userPubkey.toString("hex")}
        ${bithivePubkey.toString("hex")}
        OP_2
        OP_CHECKMULTISIG
    OP_ENDIF
    `
      .trim()
      .replace(/\s+/g, " "),
  );
}

function reconstructSignature(big_r: string, big_s: string) {
  const r = big_r.slice(2).padStart(64, "0");
  const s = big_s.padStart(64, "0");

  const rawSignature = Buffer.from(r + s, "hex");

  if (rawSignature.length !== 64) {
    throw new Error("Invalid signature length.");
  }

  return rawSignature;
}

export function buildBitHiveSignature(bigR: string, s: string) {
  return bitcoin.script.signature.encode(
    reconstructSignature(bigR, s),
    bitcoin.Transaction.SIGHASH_ALL,
  );
}

export function multisigWithdrawScript(
  userSig: Buffer,
  bithiveSig: Buffer,
): Buffer {
  return bitcoin.script.compile([
    bitcoin.opcodes.OP_0,
    userSig,
    bithiveSig,
    bitcoin.opcodes.OP_FALSE,
  ]);
}
