export interface User {
  id: string;
  username: string;
  displayName: string;
  role: "admin" | "teacher";
}

export interface AuthStatus {
  setupRequired: boolean;
  user: User | null;
}
