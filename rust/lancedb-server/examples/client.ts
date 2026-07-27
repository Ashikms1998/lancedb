export type Document = {
  id: string;
  text: string;
  vector: number[];
};

export type SearchHit = Document & { distance: number };

export class LanceServerClient {
  constructor(private readonly baseUrl = "http://127.0.0.1:8080/v1") {}

  async ping(): Promise<string> {
    const response = await this.request<{ message: string }>("/ping");
    return response.message;
  }

  async health(): Promise<{ status: string; database: string }> {
    return this.request("/db/health");
  }

  async listTables(): Promise<string[]> {
    const response = await this.request<{ tables: string[] }>("/tables");
    return response.tables;
  }

  async createTable(name: string, vectorDimension: number): Promise<void> {
    await this.request("/tables", {
      method: "POST",
      body: JSON.stringify({ name, vector_dimension: vectorDimension }),
    });
  }

  async insert(table: string, rows: Document[]): Promise<number> {
    const response = await this.request<{ count: number }>(
      `/tables/${encodeURIComponent(table)}/rows`,
      { method: "POST", body: JSON.stringify({ rows }) },
    );
    return response.count;
  }

  async getRows(table: string, limit = 100): Promise<Document[]> {
    const response = await this.request<{ rows: Document[] }>(
      `/tables/${encodeURIComponent(table)}/rows?limit=${limit}`,
    );
    return response.rows;
  }

  async countRows(table: string): Promise<number> {
    const response = await this.request<{ count: number }>(
      `/tables/${encodeURIComponent(table)}/count`,
    );
    return response.count;
  }

  async search(
    table: string,
    vector: number[],
    limit = 10,
  ): Promise<SearchHit[]> {
    const response = await this.request<{ rows: SearchHit[] }>(
      `/tables/${encodeURIComponent(table)}/search`,
      {
        method: "POST",
        body: JSON.stringify({ vector, limit }),
      },
    );
    return response.rows;
  }

  async updateText(table: string, id: string, text: string): Promise<number> {
    const response = await this.request<{ count: number }>(
      `/tables/${encodeURIComponent(table)}/rows/${encodeURIComponent(id)}`,
      { method: "PATCH", body: JSON.stringify({ text }) },
    );
    return response.count;
  }

  async deleteRow(table: string, id: string): Promise<number> {
    const response = await this.request<{ count: number }>(
      `/tables/${encodeURIComponent(table)}/rows/${encodeURIComponent(id)}`,
      { method: "DELETE" },
    );
    return response.count;
  }

  async dropTable(table: string): Promise<void> {
    await this.request<void>(`/tables/${encodeURIComponent(table)}`, {
      method: "DELETE",
    });
  }

  private async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const response = await fetch(`${this.baseUrl}${path}`, {
      ...init,
      headers: { "content-type": "application/json", ...init.headers },
    });
    if (response.status === 204) {
      return undefined as T;
    }
    const payload = await response.json();
    if (!response.ok || !payload.ok) {
      throw new Error(payload.error?.message ?? `HTTP ${response.status}`);
    }
    return payload.data as T;
  }
}
