import type {
  Character,
  Costume,
  Prop,
  Scene,
} from "@/lib/bindings/commands";

/**
 * Common shape exposed by the four subject list/card paths. Every subject
 * carries id, name, description, and a nullable reference image — that's all
 * the grid/card needs. Concrete forms re-narrow to the original type.
 */
export type SubjectListItem = Character | Scene | Prop | Costume;
