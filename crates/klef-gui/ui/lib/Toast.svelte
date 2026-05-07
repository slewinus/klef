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
    bottom: 14px;
    left: 50%;
    transform: translateX(-50%);
    padding: 7px 14px;
    background: var(--text);
    color: #fff;
    border-radius: var(--radius);
    font-size: 12px;
    font-weight: 500;
    opacity: 0;
    pointer-events: none;
    transition: opacity 120ms;
    box-shadow: var(--shadow);
    white-space: nowrap;
  }
  .toast.show { opacity: 0.95; }
</style>
