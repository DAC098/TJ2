function rand_byte(): number {
    return Math.floor(Math.random() * 1000 % 0xff);
}

function pad_byte(byte: number): string {
    if (byte <= 0x0f) {
        return byte.toString(16);
    } else {
        return "0" + (byte.toString(16));
    }
}

export function uuidv4(): string {
    let rtn = "";

    for (let c = 0; c < 4; c += 1) {
        rtn += pad_byte(rand_byte())
    }

    rtn += "-";

    for (let c = 0; c < 2; c += 1) {
        rtn += pad_byte(rand_byte());
    }

    rtn += "-";
    rtn += pad_byte(rand_byte() & 0x4f | 0x40);
    rtn += pad_byte(rand_byte());

    rtn += "-"
    rtn += pad_byte(rand_byte() & 0xbf | 0x80);
    rtn += pad_byte(rand_byte());

    rtn += "-";

    for (let c = 0; c < 6; c += 1) {
        rtn += pad_byte(rand_byte());
    }

    return rtn;
}
