import {EventEmitter, once} from "events";
import assert from "assert";

export interface SpecificEmitter<Label extends string | symbol, MsgType extends any[]> extends EventEmitter {
  on(event: Label, listener: (...msg: MsgType) => void): this;
}

type EventArgs<T extends EventEmitter, Label extends string | symbol> =
  T extends SpecificEmitter<Label, infer U> ? U : never;

/**
 * Returns an async iterator over each emitted event.
 * @param source
 * @param event
 */
export function eachEvent<
  T extends SpecificEmitter<Event, any>,
  Event extends string | symbol
>(
  source: T,
  event: Event
) {
  const items: EventArgs<T, Event>[] = [];
  const sync = new EventEmitter();
  source.on(event, (...args: EventArgs<T, Event>) => {
    items.push(args);
    if (items.length === 1)
      sync.emit('fill');
  });
  return {
    [Symbol.asyncIterator]() {
      return {
        async next() {
          if (!items.length)
            await once(sync, 'fill');

          return {done: false, value: items.shift()!};
        }
      }
    }
  }
}

/**
 * Returns an async iterator over each emitted event.
 */
export function followEvents<T extends (string|symbol)>(source: EventEmitter, ...events: T[]): AsyncIterable<[T, ...any[]]> {
  const sync = new EventEmitter();
  const items: any[] = [];

  for (const event of events)
    source.on(event, (...args) => {
      items.push([event, ...args]);
      if (items.length === 1)
        sync.emit('fill');
    });

  return {
    [Symbol.asyncIterator]() {
      return {
        async next() {
          if (!items.length)
            await once(sync, 'fill');

          return {done: false, value: items.shift()!};
        }
      }
    }
  }
}

export function assertTypeNever(arg: never): never {
  assert.fail('Unreachable');
}
