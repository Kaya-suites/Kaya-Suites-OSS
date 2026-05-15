export type Role = "user" | "assistant";

export type CitationRef = {
  label: number;
  docId: string;
  paragraphId: string;
  title: string;
};

export type ProposedEdit = {
  id: string;
  docId: string;
  paragraphId: string;
  original: string;
  proposed: string;
  status: "pending" | "approved" | "rejected";
};

export type ProposedDelete = {
  id: string;
  docId: string;
  docTitle: string;
  status: "pending" | "approved" | "rejected";
};

export type ChatMessageData = {
  id: string;
  role: Role;
  content: string;
  citations: CitationRef[];
  proposedEdits?: ProposedEdit[];
  proposedDeletes?: ProposedDelete[];
  timestamp: number;
};

export type ChatSession = {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  messageCount: number;
};

export type KayaDocument = {
  id: string;
  title: string;
  body: string;
  tags: string[];
  lastReviewed?: string;
};

export type SSEEvent =
  | { type: "TextChunk"; content: string }
  | { type: "CitationFound"; docId: string; paragraphId: string; label: number; title: string }
  | { type: "ProposedEditEmitted"; editId: string; docId: string; paragraphId: string; original: string; proposed: string }
  | { type: "ProposedDeleteEmitted"; editId: string; docId: string; docTitle: string }
  | { type: "SessionRenamed"; sessionId: string; title: string }
  | { type: "Done" }
  | { type: "Error"; message: string };
