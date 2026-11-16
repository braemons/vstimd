// Exercise uncertain command outcomes without a daemon or wall-clock sleeps.
import { create, toBinary } from "@bufbuild/protobuf";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { CommandTransport } from "../src/transport.js";
import { RequestSchema, ResponseSchema } from "../src/_proto/vstimd/v1/service_pb.js";

class Socket extends EventTarget {
  static OPEN = 1;
  static latest: Socket;
  readyState = Socket.OPEN;
  binaryType = "";
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;
  send = vi.fn();

  constructor(_url: string) {
    super();
    Socket.latest = this;
    Promise.resolve().then(() => this.onopen?.());
  }

  close(): void {
    this.readyState = 3;
    this.onclose?.();
    this.dispatchEvent(new Event("close"));
  }

  reply(): void {
    const response = toBinary(ResponseSchema, create(ResponseSchema));
    this.dispatchEvent(new MessageEvent("message", { data: response.buffer }));
  }
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("WebSocket", Socket);
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

it("serializes commands until the preceding response arrives", async () => {
  const transport = await CommandTransport.connect("ws://test");
  const first = transport.request(create(RequestSchema));
  const second = transport.request(create(RequestSchema));
  await Promise.resolve();
  expect(Socket.latest.send).toHaveBeenCalledTimes(1);
  Socket.latest.reply();
  await first;
  await Promise.resolve();
  expect(Socket.latest.send).toHaveBeenCalledTimes(2);
  Socket.latest.reply();
  await second;
  transport.close();
});

it("closes a timed-out channel so a late response cannot satisfy a queued command", async () => {
  const transport = await CommandTransport.connect("ws://test");
  const first = expect(transport.request(create(RequestSchema))).rejects.toThrow("did not reply");
  const second = expect(transport.request(create(RequestSchema))).rejects.toThrow("closed");
  await vi.advanceTimersByTimeAsync(10000);
  await first;
  Socket.latest.reply();
  await second;
  expect(Socket.latest.send).toHaveBeenCalledTimes(1);
  expect(Socket.latest.readyState).toBe(3);
});

it("closes an interrupted channel before allowing another command", async () => {
  const transport = await CommandTransport.connect("ws://test");
  const failed = expect(transport.request(create(RequestSchema))).rejects.toThrow("interrupted");
  await Promise.resolve();
  Socket.latest.dispatchEvent(new Event("error"));
  await failed;
  await expect(transport.request(create(RequestSchema))).rejects.toThrow("closed");
  expect(Socket.latest.send).toHaveBeenCalledTimes(1);
});
