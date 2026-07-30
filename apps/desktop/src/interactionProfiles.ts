import type { InteractionProfile } from "./types";

export interface InteractionProfileOption {
  value: InteractionProfile;
  label: string;
  description: string;
}

export const INTERACTION_PROFILE_OPTIONS = [
  {
    value: "none",
    label: "No Interaction",
    description: "0% modeled disruption at every turn.",
  },
  {
    value: "light",
    label: "Mild Interaction",
    description: "From turn 3 · 3% engine disruption · 8% attempt stop.",
  },
  {
    value: "typical",
    label: "Moderate Interaction",
    description: "From turn 2 · 8% engine disruption · 22% attempt stop.",
  },
  {
    value: "highPower",
    label: "cEDH Interaction",
    description: "From turn 1 · 14% engine disruption · 40% attempt stop.",
  },
] as const satisfies readonly InteractionProfileOption[];

export function isInteractionProfile(value: unknown): value is InteractionProfile {
  return INTERACTION_PROFILE_OPTIONS.some((option) => option.value === value);
}

export function interactionProfileLabel(profile: InteractionProfile): string {
  return INTERACTION_PROFILE_OPTIONS.find((option) => option.value === profile)?.label
    ?? profile;
}
