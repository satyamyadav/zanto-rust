let current = "linux";
export function platform(): string { return current; }
export function setPlatform(p: string): void { current = p; }
