/**
 * SDK types: Arm, PullResult, and internal cache structures.
 */

/** An arm returned to the user after pull(). Immutable. */
export interface Arm {
  readonly armId: number;
  readonly modelName: string;
  readonly modelProvider: string;
  readonly systemPrompt: string;
  readonly isPromptTemplated: boolean;
  /** Convenience alias for modelName. */
  readonly model: string;
  /** Convenience alias for systemPrompt. */
  readonly prompt: string;
}

/** Returned by pull(), passed to update(). Immutable. */
export interface PullResult {
  readonly arm: Arm;
  readonly eventId: string;
  readonly banditId: number;
  readonly banditName: string;
  readonly scores: Readonly<Record<number, number>>;
  /** Convenience reach-through to arm.modelName. */
  readonly model: string;
  /** Convenience reach-through to arm.systemPrompt. */
  readonly prompt: string;
  /** @internal perf timestamp */
  readonly _pullTime: number;
}

/** Create a frozen Arm from raw wire data. */
export function createArm(data: {
  arm_id: number;
  model_name: string;
  model_provider: string;
  system_prompt: string;
  is_prompt_templated?: boolean;
}): Arm {
  const arm: Arm = {
    armId: data.arm_id,
    modelName: data.model_name,
    modelProvider: data.model_provider,
    systemPrompt: data.system_prompt,
    isPromptTemplated: data.is_prompt_templated ?? false,
    get model() {
      return this.modelName;
    },
    get prompt() {
      return this.systemPrompt;
    },
  };
  return Object.freeze(arm);
}

/** Create a frozen PullResult. */
export function createPullResult(data: {
  arm: Arm;
  eventId: string;
  banditId: number;
  banditName: string;
  scores: Record<number, number>;
  pullTime: number;
}): PullResult {
  const result: PullResult = {
    arm: data.arm,
    eventId: data.eventId,
    banditId: data.banditId,
    banditName: data.banditName,
    scores: Object.freeze({ ...data.scores }),
    get model() {
      return this.arm.modelName;
    },
    get prompt() {
      return this.arm.systemPrompt;
    },
    _pullTime: data.pullTime,
  };
  return Object.freeze(result);
}

/** Raw arm data from sync response (snake_case wire format). */
export interface ArmWire {
  arm_id: number;
  model_name: string;
  model_provider: string;
  system_prompt: string;
  is_prompt_templated: boolean;
  is_active: boolean;
  avg_latency_last_n: number | null;
}

/** Internal mutable cache for a bandit's state. */
export interface BanditCache {
  banditId: number;
  name: string;
  arms: Arm[]; // active only
  armWire: ArmWire[]; // all arms (for engine JSON)
  optimizationMode: string;
  avgLatencyLastN: number | null;
  budget: number | null;
  totalCost: number | null;
}
