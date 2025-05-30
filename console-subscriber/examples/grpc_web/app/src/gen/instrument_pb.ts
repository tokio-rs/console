// @generated by protoc-gen-es v1.7.2 with parameter "target=ts"
// @generated from file instrument.proto (package rs.tokio.console.instrument, syntax proto3)
/* eslint-disable */
// @ts-nocheck

import type { BinaryReadOptions, FieldList, JsonReadOptions, JsonValue, PartialMessage, PlainMessage } from "@bufbuild/protobuf";
import { Message, proto3, Timestamp } from "@bufbuild/protobuf";
import { Id, RegisterMetadata } from "./common_pb.js";
import { TaskUpdate } from "./tasks_pb.js";
import { ResourceUpdate } from "./resources_pb.js";
import { AsyncOpUpdate } from "./async_ops_pb.js";

/**
 * The time "state" of the aggregator.
 *
 * @generated from enum rs.tokio.console.instrument.Temporality
 */
export enum Temporality {
  /**
   * The aggregator is currently live.
   *
   * @generated from enum value: LIVE = 0;
   */
  LIVE = 0,

  /**
   * The aggregator is currently paused.
   *
   * @generated from enum value: PAUSED = 1;
   */
  PAUSED = 1,
}
// Retrieve enum metadata with: proto3.getEnumType(Temporality)
proto3.util.setEnumType(Temporality, "rs.tokio.console.instrument.Temporality", [
  { no: 0, name: "LIVE" },
  { no: 1, name: "PAUSED" },
]);

/**
 * InstrumentRequest requests the stream of updates
 * to observe the async runtime state over time.
 *
 * TODO: In the future allow for the request to specify
 * only the data that the caller cares about (i.e. only
 * tasks but no resources)
 *
 * @generated from message rs.tokio.console.instrument.InstrumentRequest
 */
export class InstrumentRequest extends Message<InstrumentRequest> {
  constructor(data?: PartialMessage<InstrumentRequest>) {
    super();
    proto3.util.initPartial(data, this);
  }

  static readonly runtime: typeof proto3 = proto3;
  static readonly typeName = "rs.tokio.console.instrument.InstrumentRequest";
  static readonly fields: FieldList = proto3.util.newFieldList(() => [
  ]);

  static fromBinary(bytes: Uint8Array, options?: Partial<BinaryReadOptions>): InstrumentRequest {
    return new InstrumentRequest().fromBinary(bytes, options);
  }

  static fromJson(jsonValue: JsonValue, options?: Partial<JsonReadOptions>): InstrumentRequest {
    return new InstrumentRequest().fromJson(jsonValue, options);
  }

  static fromJsonString(jsonString: string, options?: Partial<JsonReadOptions>): InstrumentRequest {
    return new InstrumentRequest().fromJsonString(jsonString, options);
  }

  static equals(a: InstrumentRequest | PlainMessage<InstrumentRequest> | undefined, b: InstrumentRequest | PlainMessage<InstrumentRequest> | undefined): boolean {
    return proto3.util.equals(InstrumentRequest, a, b);
  }
}

/**
 * TaskDetailsRequest requests the stream of updates about
 * the specific task identified in the request.
 *
 * @generated from message rs.tokio.console.instrument.TaskDetailsRequest
 */
export class TaskDetailsRequest extends Message<TaskDetailsRequest> {
  /**
   * Identifies the task for which details were requested.
   *
   * @generated from field: rs.tokio.console.common.Id id = 1;
   */
  id?: Id;

  constructor(data?: PartialMessage<TaskDetailsRequest>) {
    super();
    proto3.util.initPartial(data, this);
  }

  static readonly runtime: typeof proto3 = proto3;
  static readonly typeName = "rs.tokio.console.instrument.TaskDetailsRequest";
  static readonly fields: FieldList = proto3.util.newFieldList(() => [
    { no: 1, name: "id", kind: "message", T: Id },
  ]);

  static fromBinary(bytes: Uint8Array, options?: Partial<BinaryReadOptions>): TaskDetailsRequest {
    return new TaskDetailsRequest().fromBinary(bytes, options);
  }

  static fromJson(jsonValue: JsonValue, options?: Partial<JsonReadOptions>): TaskDetailsRequest {
    return new TaskDetailsRequest().fromJson(jsonValue, options);
  }

  static fromJsonString(jsonString: string, options?: Partial<JsonReadOptions>): TaskDetailsRequest {
    return new TaskDetailsRequest().fromJsonString(jsonString, options);
  }

  static equals(a: TaskDetailsRequest | PlainMessage<TaskDetailsRequest> | undefined, b: TaskDetailsRequest | PlainMessage<TaskDetailsRequest> | undefined): boolean {
    return proto3.util.equals(TaskDetailsRequest, a, b);
  }
}

/**
 * PauseRequest requests the stream of updates to pause.
 *
 * @generated from message rs.tokio.console.instrument.PauseRequest
 */
export class PauseRequest extends Message<PauseRequest> {
  constructor(data?: PartialMessage<PauseRequest>) {
    super();
    proto3.util.initPartial(data, this);
  }

  static readonly runtime: typeof proto3 = proto3;
  static readonly typeName = "rs.tokio.console.instrument.PauseRequest";
  static readonly fields: FieldList = proto3.util.newFieldList(() => [
  ]);

  static fromBinary(bytes: Uint8Array, options?: Partial<BinaryReadOptions>): PauseRequest {
    return new PauseRequest().fromBinary(bytes, options);
  }

  static fromJson(jsonValue: JsonValue, options?: Partial<JsonReadOptions>): PauseRequest {
    return new PauseRequest().fromJson(jsonValue, options);
  }

  static fromJsonString(jsonString: string, options?: Partial<JsonReadOptions>): PauseRequest {
    return new PauseRequest().fromJsonString(jsonString, options);
  }

  static equals(a: PauseRequest | PlainMessage<PauseRequest> | undefined, b: PauseRequest | PlainMessage<PauseRequest> | undefined): boolean {
    return proto3.util.equals(PauseRequest, a, b);
  }
}

/**
 * ResumeRequest requests the stream of updates to resume after a pause.
 *
 * @generated from message rs.tokio.console.instrument.ResumeRequest
 */
export class ResumeRequest extends Message<ResumeRequest> {
  constructor(data?: PartialMessage<ResumeRequest>) {
    super();
    proto3.util.initPartial(data, this);
  }

  static readonly runtime: typeof proto3 = proto3;
  static readonly typeName = "rs.tokio.console.instrument.ResumeRequest";
  static readonly fields: FieldList = proto3.util.newFieldList(() => [
  ]);

  static fromBinary(bytes: Uint8Array, options?: Partial<BinaryReadOptions>): ResumeRequest {
    return new ResumeRequest().fromBinary(bytes, options);
  }

  static fromJson(jsonValue: JsonValue, options?: Partial<JsonReadOptions>): ResumeRequest {
    return new ResumeRequest().fromJson(jsonValue, options);
  }

  static fromJsonString(jsonString: string, options?: Partial<JsonReadOptions>): ResumeRequest {
    return new ResumeRequest().fromJsonString(jsonString, options);
  }

  static equals(a: ResumeRequest | PlainMessage<ResumeRequest> | undefined, b: ResumeRequest | PlainMessage<ResumeRequest> | undefined): boolean {
    return proto3.util.equals(ResumeRequest, a, b);
  }
}

/**
 * Update carries all information regarding tasks, resources, async operations
 * and resource operations in one message. There are a couple of reasons to combine all
 * of these into a single message:
 *
 * - we can use one single timestamp for all the data
 * - we can have all the new_metadata in one place
 * - things such as async ops and resource ops do not make sense
 *   on their own as they have relations to tasks and resources
 *
 * @generated from message rs.tokio.console.instrument.Update
 */
export class Update extends Message<Update> {
  /**
   * The system time when this update was recorded.
   *
   * This is the timestamp any durations in the included `Stats` were
   * calculated relative to.
   *
   * @generated from field: google.protobuf.Timestamp now = 1;
   */
  now?: Timestamp;

  /**
   * Task state update.
   *
   * @generated from field: rs.tokio.console.tasks.TaskUpdate task_update = 2;
   */
  taskUpdate?: TaskUpdate;

  /**
   * Resource state update.
   *
   * @generated from field: rs.tokio.console.resources.ResourceUpdate resource_update = 3;
   */
  resourceUpdate?: ResourceUpdate;

  /**
   * Async operations state update
   *
   * @generated from field: rs.tokio.console.async_ops.AsyncOpUpdate async_op_update = 4;
   */
  asyncOpUpdate?: AsyncOpUpdate;

  /**
   * Any new span metadata that was registered since the last update.
   *
   * @generated from field: rs.tokio.console.common.RegisterMetadata new_metadata = 5;
   */
  newMetadata?: RegisterMetadata;

  constructor(data?: PartialMessage<Update>) {
    super();
    proto3.util.initPartial(data, this);
  }

  static readonly runtime: typeof proto3 = proto3;
  static readonly typeName = "rs.tokio.console.instrument.Update";
  static readonly fields: FieldList = proto3.util.newFieldList(() => [
    { no: 1, name: "now", kind: "message", T: Timestamp },
    { no: 2, name: "task_update", kind: "message", T: TaskUpdate },
    { no: 3, name: "resource_update", kind: "message", T: ResourceUpdate },
    { no: 4, name: "async_op_update", kind: "message", T: AsyncOpUpdate },
    { no: 5, name: "new_metadata", kind: "message", T: RegisterMetadata },
  ]);

  static fromBinary(bytes: Uint8Array, options?: Partial<BinaryReadOptions>): Update {
    return new Update().fromBinary(bytes, options);
  }

  static fromJson(jsonValue: JsonValue, options?: Partial<JsonReadOptions>): Update {
    return new Update().fromJson(jsonValue, options);
  }

  static fromJsonString(jsonString: string, options?: Partial<JsonReadOptions>): Update {
    return new Update().fromJsonString(jsonString, options);
  }

  static equals(a: Update | PlainMessage<Update> | undefined, b: Update | PlainMessage<Update> | undefined): boolean {
    return proto3.util.equals(Update, a, b);
  }
}

/**
 * StateRequest requests the current state of the aggregator.
 *
 * @generated from message rs.tokio.console.instrument.StateRequest
 */
export class StateRequest extends Message<StateRequest> {
  constructor(data?: PartialMessage<StateRequest>) {
    super();
    proto3.util.initPartial(data, this);
  }

  static readonly runtime: typeof proto3 = proto3;
  static readonly typeName = "rs.tokio.console.instrument.StateRequest";
  static readonly fields: FieldList = proto3.util.newFieldList(() => [
  ]);

  static fromBinary(bytes: Uint8Array, options?: Partial<BinaryReadOptions>): StateRequest {
    return new StateRequest().fromBinary(bytes, options);
  }

  static fromJson(jsonValue: JsonValue, options?: Partial<JsonReadOptions>): StateRequest {
    return new StateRequest().fromJson(jsonValue, options);
  }

  static fromJsonString(jsonString: string, options?: Partial<JsonReadOptions>): StateRequest {
    return new StateRequest().fromJsonString(jsonString, options);
  }

  static equals(a: StateRequest | PlainMessage<StateRequest> | undefined, b: StateRequest | PlainMessage<StateRequest> | undefined): boolean {
    return proto3.util.equals(StateRequest, a, b);
  }
}

/**
 * State carries the current state of the aggregator.
 *
 * @generated from message rs.tokio.console.instrument.State
 */
export class State extends Message<State> {
  /**
   * @generated from field: rs.tokio.console.instrument.Temporality temporality = 1;
   */
  temporality = Temporality.LIVE;

  constructor(data?: PartialMessage<State>) {
    super();
    proto3.util.initPartial(data, this);
  }

  static readonly runtime: typeof proto3 = proto3;
  static readonly typeName = "rs.tokio.console.instrument.State";
  static readonly fields: FieldList = proto3.util.newFieldList(() => [
    { no: 1, name: "temporality", kind: "enum", T: proto3.getEnumType(Temporality) },
  ]);

  static fromBinary(bytes: Uint8Array, options?: Partial<BinaryReadOptions>): State {
    return new State().fromBinary(bytes, options);
  }

  static fromJson(jsonValue: JsonValue, options?: Partial<JsonReadOptions>): State {
    return new State().fromJson(jsonValue, options);
  }

  static fromJsonString(jsonString: string, options?: Partial<JsonReadOptions>): State {
    return new State().fromJsonString(jsonString, options);
  }

  static equals(a: State | PlainMessage<State> | undefined, b: State | PlainMessage<State> | undefined): boolean {
    return proto3.util.equals(State, a, b);
  }
}

/**
 * `PauseResponse` is the value returned after a pause request.
 *
 * @generated from message rs.tokio.console.instrument.PauseResponse
 */
export class PauseResponse extends Message<PauseResponse> {
  constructor(data?: PartialMessage<PauseResponse>) {
    super();
    proto3.util.initPartial(data, this);
  }

  static readonly runtime: typeof proto3 = proto3;
  static readonly typeName = "rs.tokio.console.instrument.PauseResponse";
  static readonly fields: FieldList = proto3.util.newFieldList(() => [
  ]);

  static fromBinary(bytes: Uint8Array, options?: Partial<BinaryReadOptions>): PauseResponse {
    return new PauseResponse().fromBinary(bytes, options);
  }

  static fromJson(jsonValue: JsonValue, options?: Partial<JsonReadOptions>): PauseResponse {
    return new PauseResponse().fromJson(jsonValue, options);
  }

  static fromJsonString(jsonString: string, options?: Partial<JsonReadOptions>): PauseResponse {
    return new PauseResponse().fromJsonString(jsonString, options);
  }

  static equals(a: PauseResponse | PlainMessage<PauseResponse> | undefined, b: PauseResponse | PlainMessage<PauseResponse> | undefined): boolean {
    return proto3.util.equals(PauseResponse, a, b);
  }
}

/**
 * `ResumeResponse` is the value returned after a resume request.
 *
 * @generated from message rs.tokio.console.instrument.ResumeResponse
 */
export class ResumeResponse extends Message<ResumeResponse> {
  constructor(data?: PartialMessage<ResumeResponse>) {
    super();
    proto3.util.initPartial(data, this);
  }

  static readonly runtime: typeof proto3 = proto3;
  static readonly typeName = "rs.tokio.console.instrument.ResumeResponse";
  static readonly fields: FieldList = proto3.util.newFieldList(() => [
  ]);

  static fromBinary(bytes: Uint8Array, options?: Partial<BinaryReadOptions>): ResumeResponse {
    return new ResumeResponse().fromBinary(bytes, options);
  }

  static fromJson(jsonValue: JsonValue, options?: Partial<JsonReadOptions>): ResumeResponse {
    return new ResumeResponse().fromJson(jsonValue, options);
  }

  static fromJsonString(jsonString: string, options?: Partial<JsonReadOptions>): ResumeResponse {
    return new ResumeResponse().fromJsonString(jsonString, options);
  }

  static equals(a: ResumeResponse | PlainMessage<ResumeResponse> | undefined, b: ResumeResponse | PlainMessage<ResumeResponse> | undefined): boolean {
    return proto3.util.equals(ResumeResponse, a, b);
  }
}

