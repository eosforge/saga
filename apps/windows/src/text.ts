/** Shared paragraph projection for reading and sentence navigation. */
import type { Section } from "../../../protocol/bindings/Section";

/** One source paragraph with chapter-local, zero-based sentence positions. */
export interface Paragraph {
  /** Nonempty paragraph position in the chapter. */
  pIndex: number;
  /** Stable display identities and sentence positions. */
  chunks: { id: string; index: number; text: string }[];
}

/** Split punctuation-delimited sentences, retaining closing quotes and punctuation. */
export function sentences(body: string): string[] {
  return body.split(/\r?\n/).flatMap((line) => {
    return (line.trim().match(/[^。！？；!?]+[。！？；!?]*["'”’）)』」]*|[。！？；!?]+/gu) || [])
      .map((part) => part.trim()).filter(Boolean);
  });
}

/** Project stored chunks or derive display chunks without mutating source text. */
export function paragraphs(section: Section): Paragraph[] {
  const groups: Paragraph[] = [];
  if (section.chunks.length > 0) {
    for (const chunk of section.chunks) {
      let group = groups.at(-1);
      if (!group || group.pIndex !== chunk.paragraph) {
        group = { pIndex: chunk.paragraph, chunks: [] };
        groups.push(group);
      }
      group.chunks.push({ id: chunk.id, index: chunk.index, text: chunk.text });
    }
    return groups;
  }

  let index = 0;
  for (const line of section.body.split(/\r?\n/)) {
    const parts = sentences(line);
    if (parts.length === 0) continue;
    const pIndex = groups.length;
    const chunks = parts.map((text) => {
      const position = index++;
      return { id: `${section.id}:p${pIndex}:c${position}`, index: position, text };
    });
    groups.push({ pIndex, chunks });
  }
  return groups;
}

/** Return the same sentence sequence that the reader renders. */
export function contents(section: Section): string[] {
  return section.chunks.length > 0 ? section.chunks.map((chunk) => chunk.text) : sentences(section.body);
}
