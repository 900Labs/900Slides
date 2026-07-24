<script lang="ts">
  let message = $state('900Slides')
</script>

<main>
  <h1>{message}</h1>
</main>

<style>
  main {
    display: flex;
    justify-content: center;
    align-items: center;
    height: 100vh;
    margin: 0;
  }
  h1 {
    font-family: system-ui, sans-serif;
  }
</style>
