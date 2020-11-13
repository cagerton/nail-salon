import {once} from 'events';
import assert from 'assert';
import {Worker} from 'worker_threads';
import {ResizeRequest, WorkerResult} from './types';


export class SimpleImageWorker {
  protected sync = new FiFoExecutor();
  protected worker?: Worker;
  protected workerErr?: Promise<never>;
  protected taskId = 0;

  private ensureStarted(): asserts this is SimpleImageWorker & {worker: Worker, workerErr: Promise<never>} {
    if (!this.worker) {
      const filename = `${__dirname}/worker.js`;
      this.worker = new Worker(filename);
      this.workerErr = new Promise((resolve, reject) => {
        this.worker!.on('error', err => reject(err));
        this.worker!.on('exit', code => reject(new Error('worker died' + code)));
      }).finally(() => {
        this.worker?.terminate();
        this.worker = undefined;
      }) as Promise<never>;
    }
  }

  protected async convert_internal(req: ResizeRequest, transferBuffer: boolean) {
    this.ensureStarted();
    let thisTask = ++this.taskId;

    const transferList = transferBuffer ? [req.input] : [];
    this.worker.postMessage({taskId: thisTask, req}, transferList);

    const msgP = once(this.worker, 'message') as Promise<[WorkerResult]>;
    const [res] = await Promise.race([msgP, this.workerErr!]);
    assert.strictEqual(res.taskId, thisTask);

    if (res.err)
      throw res.err;

    return res.res!;
  }

  async convert(req: ResizeRequest, transferBuffer = false) {
    return this.sync.task(() => this.convert_internal(req, transferBuffer));
  }
}


class FiFoExecutor {
  protected running: Promise<void> = Promise.resolve();

  async task<T>(runner: () => Promise<T>): Promise<T> {
    const prev = this.running;
    const task = prev.then(() => runner());
    this.running = task.then(() => {}, () => {});
    return await task;
  }
}
