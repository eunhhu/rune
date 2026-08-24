const HEADER_WORDS = 4;
const WRITE_INDEX = 0;
const READ_INDEX = 1;
const DROPPED_INDEX = 2;
const CLOSED_INDEX = 3;

/**
 * Fixed-record single-producer/single-consumer ring over SharedArrayBuffer.
 * Push/pop allocate nothing when the caller reuses its record arrays.
 */
export class SpscInt32Ring {
  readonly buffer: SharedArrayBuffer;
  readonly capacity: number;
  readonly recordWords: number;
  readonly header: Int32Array;
  readonly records: Int32Array;
  readonly #mask: number;

  constructor(capacity: number, recordWords: number, buffer?: SharedArrayBuffer) {
    if (capacity < 2 || (capacity & (capacity - 1)) !== 0) {
      throw new RangeError("capacity must be a power of two and at least 2");
    }
    if (!Number.isInteger(recordWords) || recordWords <= 0) {
      throw new RangeError("recordWords must be a positive integer");
    }
    const words = HEADER_WORDS + capacity * recordWords;
    this.buffer = buffer ?? new SharedArrayBuffer(words * Int32Array.BYTES_PER_ELEMENT);
    if (this.buffer.byteLength !== words * Int32Array.BYTES_PER_ELEMENT) {
      throw new RangeError("shared buffer has the wrong size");
    }
    this.capacity = capacity;
    this.recordWords = recordWords;
    this.#mask = capacity - 1;
    this.header = new Int32Array(this.buffer, 0, HEADER_WORDS);
    this.records = new Int32Array(
      this.buffer,
      HEADER_WORDS * Int32Array.BYTES_PER_ELEMENT,
      capacity * recordWords,
    );
  }

  push(record: ArrayLike<number>): boolean {
    if (record.length !== this.recordWords) {
      throw new RangeError(`record must contain ${this.recordWords} words`);
    }
    if (Atomics.load(this.header, CLOSED_INDEX) !== 0) {
      return false;
    }
    const write = Atomics.load(this.header, WRITE_INDEX) >>> 0;
    const read = Atomics.load(this.header, READ_INDEX) >>> 0;
    if (write - read >= this.capacity) {
      Atomics.add(this.header, DROPPED_INDEX, 1);
      return false;
    }
    const base = (write & this.#mask) * this.recordWords;
    for (let index = 0; index < this.recordWords; index += 1) {
      this.records[base + index] = record[index] ?? 0;
    }
    Atomics.store(this.header, WRITE_INDEX, (write + 1) | 0);
    Atomics.notify(this.header, WRITE_INDEX, 1);
    return true;
  }

  pop(target: Int32Array): boolean {
    if (target.length < this.recordWords) {
      throw new RangeError(`target must contain at least ${this.recordWords} words`);
    }
    const read = Atomics.load(this.header, READ_INDEX) >>> 0;
    const write = Atomics.load(this.header, WRITE_INDEX) >>> 0;
    if (read === write) {
      return false;
    }
    const base = (read & this.#mask) * this.recordWords;
    for (let index = 0; index < this.recordWords; index += 1) {
      target[index] = this.records[base + index] ?? 0;
    }
    Atomics.store(this.header, READ_INDEX, (read + 1) | 0);
    return true;
  }

  close(): void {
    Atomics.store(this.header, CLOSED_INDEX, 1);
    Atomics.notify(this.header, WRITE_INDEX);
  }

  get dropped(): number {
    return Atomics.load(this.header, DROPPED_INDEX) >>> 0;
  }

  get size(): number {
    const write = Atomics.load(this.header, WRITE_INDEX) >>> 0;
    const read = Atomics.load(this.header, READ_INDEX) >>> 0;
    return write - read;
  }
}
