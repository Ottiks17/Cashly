<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { check } from '@tauri-apps/plugin-updater';

  const pad = (n) => String(n).padStart(2, '0');
  const todayStr = () => {
    const d = new Date();
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  };
  const rub = new Intl.NumberFormat('ru-RU', { style: 'currency', currency: 'RUB' });
  const fmt = (kopecks) => rub.format(kopecks / 100);

  let month = $state(todayStr().slice(0, 7));
  let txs = $state([]);
  let summary = $state({ income: 0, expense: 0, by_category: [] });
  let cats = $state([]);
  let models = $state([]);
  let model = $state('');
  let ollamaOk = $state(false);
  let showForm = $state(false);
  let form = $state({ date: todayStr(), type: 'expense', amount: '', category: '', description: '' });
  let formError = $state('');
  let question = $state('');
  let chat = $state([]);
  let busy = $state(false);
  let update = $state(null);
  let updating = $state('');

  const monthLabel = $derived(
    new Date(+month.slice(0, 4), +month.slice(5, 7) - 1, 1).toLocaleDateString('ru-RU', { month: 'long', year: 'numeric' })
  );
  const maxCat = $derived(Math.max(1, ...summary.by_category.map((c) => c.total)));
  const colorOf = (name) => cats.find((c) => c.name === name)?.color ?? '#888780';

  function shift(delta) {
    const dt = new Date(+month.slice(0, 4), +month.slice(5, 7) - 1 + delta, 1);
    month = `${dt.getFullYear()}-${pad(dt.getMonth() + 1)}`;
  }

  async function load(m) {
    try {
      [txs, summary] = await Promise.all([
        invoke('list_transactions', { month: m }),
        invoke('summary', { month: m })
      ]);
    } catch (err) {
      console.error(err);
    }
  }

  $effect(() => {
    load(month);
  });

  async function checkUpdate() {
    try {
      update = await check();
    } catch (err) {
      console.warn('Проверка обновлений не удалась:', err);
    }
  }

  async function installUpdate() {
    updating = 'Скачиваю…';
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (err) {
      updating = 'Не удалось обновиться, попробуйте позже';
      console.error(err);
    }
  }

  onMount(async () => {
    checkUpdate();
    cats = await invoke('list_categories');
    try {
      models = await invoke('ollama_models');
      ollamaOk = true;
      const saved = localStorage.getItem('model');
      model = models.includes(saved) ? saved : (models[0] ?? '');
    } catch {
      ollamaOk = false;
    }
  });

  async function submit(ev) {
    ev.preventDefault();
    formError = '';
    try {
      await invoke('add_transaction', { t: { ...form } });
      showForm = false;
      form = { ...form, amount: '', description: '' };
      cats = await invoke('list_categories');
      await load(month);
    } catch (err) {
      formError = String(err);
    }
  }

  async function remove(id) {
    await invoke('delete_transaction', { id });
    await load(month);
  }

  async function ask(ev) {
    ev.preventDefault();
    const q = question.trim();
    if (!q || busy) return;
    chat = [...chat, { role: 'user', text: q }];
    question = '';
    busy = true;
    try {
      if (!model) throw 'Выберите модель Ollama в боковой панели';
      const text = await invoke('ask', { question: q, model, today: todayStr() });
      chat = [...chat, { role: 'ai', text }];
    } catch (err) {
      chat = [...chat, { role: 'err', text: String(err) }];
    }
    busy = false;
  }
</script>

<div class="flex h-screen bg-neutral-50 text-sm text-neutral-900 dark:bg-neutral-900 dark:text-neutral-100">
  <aside class="flex w-48 shrink-0 flex-col gap-3 border-r border-black/10 p-3 dark:border-white/10">
    <div class="text-base font-medium">Моя бухгалтерия</div>
    <div class="mt-auto space-y-2 text-xs text-neutral-500">
      <div class="flex items-center gap-2">
        <span class="size-2 rounded-full {ollamaOk ? 'bg-green-500' : 'bg-red-500'}"></span>
        {ollamaOk ? 'Ollama online' : 'Ollama недоступна'}
      </div>
      {#if models.length}
        <select class="field text-xs" bind:value={model} onchange={() => localStorage.setItem('model', model)}>
          {#each models as m}<option value={m}>{m}</option>{/each}
        </select>
      {/if}
    </div>
  </aside>

  <main class="flex-1 space-y-4 overflow-y-auto p-5">
    {#if update}
      <div class="flex items-center justify-between rounded-lg bg-blue-600/10 px-3 py-2 text-blue-800 dark:text-blue-200">
        <span>Доступна версия {update.version}</span>
        <button class="btn" onclick={installUpdate} disabled={!!updating}>{updating || 'Обновить и перезапустить'}</button>
      </div>
    {/if}
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2 text-base font-medium">
        <button class="btn px-2 py-0.5" onclick={() => shift(-1)} aria-label="Предыдущий месяц">‹</button>
        <span class="w-40 text-center capitalize">{monthLabel}</span>
        <button class="btn px-2 py-0.5" onclick={() => shift(1)} aria-label="Следующий месяц">›</button>
      </div>
      <button class="btn" onclick={() => (showForm = true)}>+ Добавить</button>
    </div>

    <div class="grid grid-cols-3 gap-3">
      <div class="rounded-lg bg-black/5 p-3 dark:bg-white/5">
        <div class="text-xs text-neutral-500">Доходы</div>
        <div class="text-lg font-medium text-green-700 dark:text-green-400">+ {fmt(summary.income)}</div>
      </div>
      <div class="rounded-lg bg-black/5 p-3 dark:bg-white/5">
        <div class="text-xs text-neutral-500">Расходы</div>
        <div class="text-lg font-medium text-red-700 dark:text-red-400">− {fmt(summary.expense)}</div>
      </div>
      <div class="rounded-lg bg-black/5 p-3 dark:bg-white/5">
        <div class="text-xs text-neutral-500">Баланс</div>
        <div class="text-lg font-medium">{fmt(summary.income - summary.expense)}</div>
      </div>
    </div>

    <div class="grid grid-cols-[1.3fr_1fr] gap-3">
      <div class="card py-2">
        {#each txs as t (t.id)}
          <div class="group flex items-center gap-3 border-b border-black/10 py-2 last:border-0 dark:border-white/10">
            <span class="size-2 shrink-0 rounded-full" style="background:{colorOf(t.category)}"></span>
            <div class="min-w-0 flex-1">
              <div class="truncate">{t.description || t.category}</div>
              <div class="text-xs text-neutral-500">{t.date.slice(8)}.{t.date.slice(5, 7)} · {t.category}</div>
            </div>
            <span class={t.type === 'income' ? 'text-green-700 dark:text-green-400' : ''}>
              {t.type === 'income' ? '+' : '−'} {fmt(t.amount)}
            </span>
            <button class="text-neutral-400 opacity-0 hover:text-red-600 group-hover:opacity-100" onclick={() => remove(t.id)} aria-label="Удалить">✕</button>
          </div>
        {:else}
          <p class="py-6 text-center text-neutral-500">Нет операций за этот месяц. Нажмите «Добавить».</p>
        {/each}
      </div>

      <div class="card space-y-3">
        <div class="text-xs text-neutral-500">Расходы по категориям</div>
        {#each summary.by_category as c}
          <div>
            <div class="flex justify-between"><span>{c.category}</span><span>{fmt(c.total)}</span></div>
            <div class="mt-1 h-1.5 rounded bg-black/5 dark:bg-white/10">
              <div class="h-1.5 rounded" style="width:{(c.total / maxCat) * 100}%; background:{c.color ?? '#888780'}"></div>
            </div>
          </div>
        {:else}
          <p class="text-neutral-500">Пока пусто.</p>
        {/each}
      </div>
    </div>

    <div class="card space-y-2">
      <div class="text-xs text-neutral-500">Спросите о своих финансах</div>
      <div class="max-h-64 space-y-2 overflow-y-auto">
        {#each chat as m}
          <div class="flex {m.role === 'user' ? 'justify-end' : ''}">
            <div class="max-w-[80%] whitespace-pre-wrap rounded-xl px-3 py-1.5 {m.role === 'user' ? 'bg-blue-600 text-white' : m.role === 'err' ? 'bg-red-100 text-red-800 dark:bg-red-950 dark:text-red-200' : 'bg-black/5 dark:bg-white/10'}">{m.text}</div>
          </div>
        {/each}
        {#if busy}<div class="text-neutral-500">Думаю…</div>{/if}
      </div>
      <form class="flex gap-2" onsubmit={ask}>
        <input class="field" placeholder="Сколько я потратил на еду в марте?" bind:value={question} />
        <button class="btn" disabled={busy}>Спросить</button>
      </form>
    </div>
  </main>
</div>

{#if showForm}
  <div class="fixed inset-0 z-10 flex items-center justify-center bg-black/40">
    <form onsubmit={submit} class="w-96 space-y-3 rounded-xl bg-white p-5 text-neutral-900 dark:bg-neutral-800 dark:text-neutral-100">
      <div class="flex gap-2">
        {#each [['expense', 'Расход'], ['income', 'Доход']] as [v, l]}
          <button type="button" class="btn flex-1 {form.type === v ? 'bg-black/10 dark:bg-white/15' : ''}" onclick={() => (form.type = v)}>{l}</button>
        {/each}
      </div>
      <input class="field" type="date" bind:value={form.date} required />
      <input class="field" inputmode="decimal" placeholder="Сумма, ₽ (например 1500,50)" bind:value={form.amount} required />
      <input class="field" list="cats" placeholder="Категория" bind:value={form.category} required />
      <datalist id="cats">
        {#each cats.filter((c) => c.type === form.type) as c}<option value={c.name}></option>{/each}
      </datalist>
      <input class="field" placeholder="Описание" bind:value={form.description} />
      {#if formError}<p class="text-xs text-red-600">{formError}</p>{/if}
      <div class="flex justify-end gap-2">
        <button type="button" class="btn" onclick={() => (showForm = false)}>Отмена</button>
        <button class="btn border-transparent bg-blue-600 text-white hover:bg-blue-700">Сохранить</button>
      </div>
    </form>
  </div>
{/if}
