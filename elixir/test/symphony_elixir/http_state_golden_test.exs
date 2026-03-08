defmodule SymphonyElixir.HttpStateGoldenTest do
  use SymphonyElixir.TestSupport

  alias SymphonyElixirWeb.Presenter

  defmodule StaticOrchestrator do
    use GenServer

    def start_link(opts) do
      name = Keyword.fetch!(opts, :name)
      GenServer.start_link(__MODULE__, opts, name: name)
    end

    def init(opts), do: {:ok, opts}

    def handle_call(:snapshot, _from, state) do
      {:reply, Keyword.fetch!(state, :snapshot), state}
    end
  end

  test "state payload matches shared benchmark golden projection" do
    orchestrator_name = Module.concat(__MODULE__, :StaticOrchestrator)

    start_supervised!({StaticOrchestrator, name: orchestrator_name, snapshot: static_snapshot()})

    actual =
      orchestrator_name
      |> Presenter.state_payload(50)
      |> Jason.encode!()
      |> Jason.decode!()
      |> benchmark_projection()

    assert actual == load_benchmark_fixture!("benchmark_state_small.json")
  end

  defp benchmark_projection(payload) do
    running =
      payload
      |> Map.get("running", [])
      |> Enum.map(fn entry ->
        %{
          "issue_id" => entry["issue_id"],
          "issue_identifier" => entry["issue_identifier"],
          "state" => entry["state"],
          "session_id" => entry["session_id"],
          "turn_count" => entry["turn_count"],
          "last_event" => entry["last_event"],
          "last_message" => entry["last_message"],
          "tokens" => entry["tokens"]
        }
      end)
      |> Enum.sort_by(& &1["issue_id"])

    retrying =
      payload
      |> Map.get("retrying", [])
      |> Enum.map(fn entry ->
        %{
          "issue_id" => entry["issue_id"],
          "issue_identifier" => entry["issue_identifier"],
          "attempt" => entry["attempt"],
          "error" => entry["error"]
        }
      end)
      |> Enum.sort_by(& &1["issue_id"])

    %{
      "counts" => Map.get(payload, "counts"),
      "running" => running,
      "retrying" => retrying,
      "codex_totals" => Map.get(payload, "codex_totals"),
      "rate_limits" => Map.get(payload, "rate_limits")
    }
  end

  defp load_benchmark_fixture!(name) do
    __DIR__
    |> Path.join("../../../fixtures/http_state/#{name}")
    |> Path.expand()
    |> File.read!()
    |> Jason.decode!()
  end

  defp static_snapshot do
    %{
      running: [
        %{
          issue_id: "issue-http",
          identifier: "MT-HTTP",
          state: "In Progress",
          session_id: "thread-http",
          turn_count: 7,
          codex_app_server_pid: nil,
          last_codex_message: "rendered",
          last_codex_timestamp: DateTime.from_unix!(1_700_000_000),
          last_codex_event: :notification,
          codex_input_tokens: 4,
          codex_output_tokens: 8,
          codex_total_tokens: 12,
          started_at: DateTime.from_unix!(1_700_000_000)
        }
      ],
      retrying: [
        %{
          issue_id: "issue-retry",
          identifier: "MT-RETRY",
          attempt: 2,
          due_in_ms: 2_000,
          error: "boom"
        }
      ],
      codex_totals: %{input_tokens: 4, output_tokens: 8, total_tokens: 12, seconds_running: 42.5},
      rate_limits: %{"primary" => %{"remaining" => 11}}
    }
  end
end
