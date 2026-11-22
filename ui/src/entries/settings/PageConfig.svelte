<script lang="ts">
  // 设置 → 配置文件：文件位置。导出（可选内容、Key 加密）、导入、自动备份与恢复、恢复默认在 M10.4 加入。
  import { onMount } from 'svelte';
  import { clipboardWriteText, configPath, openFolder } from '$lib/ipc';
  import { toast } from '$lib/ui/toast.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Head from './Head.svelte';

  let path = $state('');
  onMount(async () => {
    path = await configPath();
  });
  const folder = $derived(path.replace(/[\\/][^\\/]*$/, ''));

  async function copyPath() {
    await clipboardWriteText(path);
    toast('已复制配置文件路径');
  }
</script>

<Head title="配置文件" desc="所有设置都存在一个 JSON 文件里（API Key 不在里面，存在系统凭据管理器中）。" />

<Card>
  <Row title="配置文件位置" desc={path || '…'}>
    <div class="btns">
      <Button variant="ghost" icon="folder" onclick={() => folder && openFolder(folder)}>打开文件夹</Button>
      <Button variant="ghost" icon="copy" onclick={copyPath}>复制路径</Button>
    </div>
  </Row>
</Card>

<style>
  .btns { display: flex; gap: 6px; }
  :global(.row .rt span) { word-break: break-all; }
</style>
