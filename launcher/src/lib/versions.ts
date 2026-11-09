// What the launcher shows. Ids must match electron/versions.js, which holds what gets installed.
// The launcher offers a single client: Minecraft 26.2. 1.21.11 and 1.16.5 are not selectable.

export interface GameVersion {
  id: string;
  title: string;
  tag: string;
  subtitle: string;
  image: string;
  description: string;
  stack: string[];
}

export const CURRENT: GameVersion = {
  id: "26.2",
  title: "26.2",
  tag: "FREE",
  subtitle: "Клиент FestVisuals",
  image: "./art/world.jpg",
  description:
    "Мы создали для вас лучший клиент, который даст вам огромное преимущество в игре. " +
    "В этом клиенте огромный функционал, который подойдет под все популярные сервера майнкрафт. " +
    "Этот клиент является стабильным, а это означает, что вы получите наилучший игровой опыт " +
    "без багов и различных ошибок.",
  stack: ["Fabric", "ViaFabricPlus", "Sodium", "Lithium", "FerriteCore"],
};

export const VERSIONS: GameVersion[] = [CURRENT];

export function findVersion(id: string | undefined): GameVersion | undefined {
  return VERSIONS.find((v) => v.id === id);
}
