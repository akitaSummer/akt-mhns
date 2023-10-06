const mhns = require("./akt-mhns.darwin-arm64");

const { Worker, isMainThread, parentPort } = require("worker_threads");
const sendPtr = async (fn) => {
  await mhns.addCallback(fn);
  parentPort.postMessage("success");
};

function fibonacci(num) {
  if (num == 1) return 0;
  if (num == 2) return 1;
  return fibonacci(num - 1) + fibonacci(num - 2);
}

const createWorker = () => {
  return new Promise((resolve, reject) => {
    const worker = new Worker("./index.js", { workerData: { num: 5 } });
    worker.once("message", (result) => {
      resolve();
    });
  });
};

if (isMainThread) {
  const list = [];
  for (let i = 0; i < 10; i++) {
    list.push(createWorker());
  }
  Promise.all(list).then(() => {
    mhns.createApp(
      9988,
      (err) => {
        if (err) {
          console.log(err);
        } else {
          console.log("ss");
        }
      },
      (err) => {
        if (err) {
          console.error(err);
        }
        fibonacci(40);
        a++;
      }
    );
  });
} else {
  const log = () => {
    fibonacci(40);
    a++;
  };

  sendPtr(log);
}
