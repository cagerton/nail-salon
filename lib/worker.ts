import assert from 'assert';
import {isMainThread, parentPort} from 'worker_threads';
import {eachEvent} from "./util";
import type {ResizeResult, WorkerRequest} from './types';

function isRequest(value: unknown): value is WorkerRequest {
  return value && typeof value === 'object' && 'taskId' in value && 'req' in value;
}

async function workerMain() {
  assert(parentPort && !isMainThread);
  const tasks = eachEvent(parentPort, 'message');
  const {convert} = await import('../wasm/nail_salon');

  for await (const [task] of tasks) {
    assert(isRequest(task));

    const {taskId, req} = task;

    let res: ResizeResult | undefined;
    try {
      res = convert(req);
      parentPort.postMessage({taskId, res});
    } catch (err) {
      parentPort.postMessage({taskId, err: new Error(err)});
    }
  }
}

if (!isMainThread && require.main === module)
  workerMain();
