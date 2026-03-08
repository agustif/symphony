defmodule SymphonyElixir.WorkflowStore do
  @moduledoc """
  Caches the last known good workflow and reloads it when `WORKFLOW.md` changes.
  """

  use GenServer
  require Logger

  alias SymphonyElixir.{Config, Workflow}

  @poll_interval_ms 1_000

  defmodule State do
    @moduledoc false

    defstruct [:path, :stamp, :workflow, :validated_options]
  end

  @spec start_link(keyword()) :: GenServer.on_start()
  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end

  @spec current() :: {:ok, Workflow.loaded_workflow()} | {:error, term()}
  def current do
    case Process.whereis(__MODULE__) do
      pid when is_pid(pid) ->
        GenServer.call(__MODULE__, :current)

      _ ->
        Workflow.load()
    end
  end

  @spec cached_validated_options() :: {:ok, map()} | :unavailable
  def cached_validated_options do
    case Process.whereis(__MODULE__) do
      pid when is_pid(pid) ->
        GenServer.call(__MODULE__, :cached_validated_options)

      _ ->
        :unavailable
    end
  end

  @spec force_reload() :: :ok | {:error, term()}
  def force_reload do
    case Process.whereis(__MODULE__) do
      pid when is_pid(pid) ->
        GenServer.call(__MODULE__, :force_reload)

      _ ->
        case Workflow.load() do
          {:ok, _workflow} -> :ok
          {:error, reason} -> {:error, reason}
        end
    end
  end

  @impl true
  def init(_opts) do
    case load_state(Workflow.workflow_file_path()) do
      {:ok, state} ->
        schedule_poll()
        {:ok, state}

      {:error, reason} ->
        {:stop, reason}
    end
  end

  @impl true
  def handle_call(:current, _from, %State{} = state) do
    path = Workflow.workflow_file_path()

    cond do
      path != state.path ->
        reply_with_reload(reload_path(path, state))

      workflow_file_unchanged?(path, state.stamp) ->
        {:reply, {:ok, state.workflow}, state}

      true ->
        reply_with_reload(reload_current_path(path, state))
    end
  end

  def handle_call(:force_reload, _from, %State{} = state) do
    case reload_state(state) do
      {:ok, new_state} ->
        {:reply, :ok, new_state}

      {:error, reason, new_state} ->
        {:reply, {:error, reason}, new_state}
    end
  end

  def handle_call(:cached_validated_options, _from, %State{} = state) do
    path = Workflow.workflow_file_path()

    cond do
      path != state.path ->
        reply_with_validated_options(reload_path(path, state))

      workflow_file_unchanged?(path, state.stamp) ->
        {:reply, {:ok, state.validated_options}, state}

      true ->
        reply_with_validated_options(reload_current_path(path, state))
    end
  end

  @impl true
  def handle_info(:poll, %State{} = state) do
    schedule_poll()

    case reload_state(state) do
      {:ok, new_state} -> {:noreply, new_state}
      {:error, _reason, new_state} -> {:noreply, new_state}
    end
  end

  defp schedule_poll do
    Process.send_after(self(), :poll, @poll_interval_ms)
  end

  defp reload_state(%State{} = state) do
    path = Workflow.workflow_file_path()

    if path != state.path do
      reload_path(path, state)
    else
      reload_current_path(path, state)
    end
  end

  defp reload_path(path, state) do
    case load_state(path) do
      {:ok, new_state} ->
        {:ok, new_state}

      {:error, reason} ->
        log_reload_error(path, reason)
        {:error, reason, state}
    end
  end

  defp reload_current_path(path, state) do
    case current_stamp(path) do
      {:ok, stamp} when stamp == state.stamp ->
        {:ok, state}

      {:ok, _stamp} ->
        reload_path(path, state)

      {:error, reason} ->
        log_reload_error(path, reason)
        {:error, reason, state}
    end
  end

  defp load_state(path) do
    with {:ok, workflow} <- Workflow.load(path),
         {:ok, stamp} <- current_stamp(path) do
      {:ok,
       %State{
         path: path,
         stamp: stamp,
         workflow: workflow,
         validated_options: Config.validated_workflow_options_from_config(workflow.config)
       }}
    else
      {:error, reason} ->
        {:error, reason}
    end
  end

  defp current_stamp(path) when is_binary(path) do
    with {:ok, {mtime, size}} <- current_file_stamp(path),
         {:ok, content} <- File.read(path) do
      {:ok, {mtime, size, :erlang.phash2(content)}}
    else
      {:error, reason} -> {:error, reason}
    end
  end

  defp workflow_file_unchanged?(path, stamp) when is_binary(path) do
    case current_stamp(path) do
      {:ok, ^stamp} -> true
      _ -> false
    end
  end

  defp workflow_file_unchanged?(_path, _stamp), do: false

  defp current_file_stamp(path) when is_binary(path) do
    case File.stat(path, time: :posix) do
      {:ok, stat} -> {:ok, {stat.mtime, stat.size}}
      {:error, reason} -> {:error, reason}
    end
  end

  defp reply_with_reload({:ok, new_state}), do: {:reply, {:ok, new_state.workflow}, new_state}
  defp reply_with_reload({:error, _reason, new_state}), do: {:reply, {:ok, new_state.workflow}, new_state}

  defp reply_with_validated_options({:ok, new_state}), do: {:reply, {:ok, new_state.validated_options}, new_state}

  defp reply_with_validated_options({:error, _reason, new_state}),
    do: {:reply, {:ok, new_state.validated_options}, new_state}

  defp log_reload_error(path, reason) do
    Logger.error("Failed to reload workflow path=#{path} reason=#{inspect(reason)}; keeping last known good configuration")
  end
end
