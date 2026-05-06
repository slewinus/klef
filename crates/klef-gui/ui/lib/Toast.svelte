<script lang="ts">
  interface Props {
    message: string | null;
  }

  let { message }: Props = $props();
</script>

<div class="toast" class:show={message !== null}>
  {message ?? ""}
</div>

<style>
  .toast {
    position: absolute;
    bottom: 16px;
    left: 50%;
    transform: translateX(-50%);
    padding: 8px 14px;
    background: #1d1d1f;
    color: white;
    border-radius: 6px;
    font-size: 12px;
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.2s;
  }
  .toast.show {
    opacity: 0.95;
  }
  @media (prefers-color-scheme: dark) {
    .toast {
      background: #f5f5f7;
      color: #1d1d1f;
    }
  }
</style>
