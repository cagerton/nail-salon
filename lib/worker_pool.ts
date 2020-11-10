import assert from 'assert'
import {EventEmitter} from 'events';
import {setTimeout} from "timers";
import {assertTypeNever, followEvents} from './util';
import {Worker} from 'worker_threads';
import type {WorkerRequest, WorkerResult, ResizeResult, ResizeRequest} from './types';

type Timeout = ReturnType<typeof setTimeout>;


interface Task extends WorkerRequest {
  resolve: (resp: ResizeResult) => void;
  reject: (err: Error) => void;
}

export class ImageWorkerPool extends EventEmitter {
  protected readonly available: Worker[] = [];
  protected readonly taskMap = new Map<Worker, Task>();
  protected readonly timers = new Map<Worker, Timeout>();

  protected taskCounter = 0;
  protected workQueue: Task[] = [];

  constructor(
    protected workerCount: number,
    protected timeLimit = 3000,
    protected path = `${__dirname}/worker.js`,
  ) {
    super();
    assert(workerCount > 0);
    assert(timeLimit > 0);
    assert(this.runPool());
  }

  async convert(req: ResizeRequest): Promise<ResizeResult> {
    return new Promise((resolve, reject) => {
      this.workQueue.push({taskId: this.taskCounter++, resolve, reject, req});
    });
  }

  protected async runPool() {
    for (let idx = 0; idx < this.workerCount; idx++)
      assert(this.runWorker());

    for await (const [name] of followEvents(this,'worker-died', 'worker-free', 'task-added')) {
      switch (name) {
        case 'worker-died':
          assert(this.runWorker());
          break;

        case 'worker-free':
        case 'task-added':
          while (this.available.length && this.workQueue.length) {
            const worker = this.available.pop()!;
            const task = this.workQueue.shift()!;
            const {taskId, req} = task;

            this.taskMap.set(worker, task);
            this.timers.set(worker, setTimeout(() => {
              task.reject(new Error(`Task took too long`));
              this.taskMap.delete(worker);
              worker.terminate();
            }, this.timeLimit));

            worker.postMessage({taskId, req});
          }
          break;

        default:
          assertTypeNever(name); // unreachable
      }
    }
  }

  protected async runWorker() {
    const worker = new Worker(this.path);

    for await (const [name, ...args] of followEvents(worker,'error', 'message', 'online', 'exit')) {
      const task = this.taskMap.get(worker);
      const timer = this.timers.get(worker);
      if (timer)
        clearTimeout(timer);

      this.timers.delete(worker);

      switch (name) {
        case 'online':
          this.available.push(worker);
          this.emit('worker-free');
          break;

        case 'message':
          this.taskMap.delete(worker);
          assert(task);

          const [result] = args as [WorkerResult];
          assert.strictEqual(result.taskId, task.taskId);
          assert(result.err || result.res);

          if (result.err)
            task.reject(result.err);
          else
            task.resolve(result.res!);

          this.available.push(worker);
          this.emit('worker-free');
          break;

        case 'error':
          const [err] = args as [Error];
          if (!task)
            throw err;

          this.taskMap.delete(worker);
          task.reject(err);

          await worker.terminate();
          break;

        case 'exit':
          this.emit('worker-died');
          break;

        default:
          assertTypeNever(name); // unreachable
      }
    }
  }
}
