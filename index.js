/**
 * Mock Data Generator: Creates 50 events with randomized dependencies.
 */
const generateMockData = (count = 50) => {
    const data = [];
  
    // 1. Create base nodes
    for (let i = 1; i <= count; i++) {
      data.push({
        id: `node_${i}`,
        is_pending: Math.random() < 0.3, // 30% are pendings
        temp_deps: []
      });
    }
  
    // 2. Assign dependencies (up to 4)
    data.forEach((node, index) => {
      if (index > 5) { // Ensure early nodes are "roots" (no dependencies)
        const depCount = Math.floor(Math.random() * 4); 
        const candidates = data.slice(0, index); // Prevents circular dependencies
        
        for (let j = 0; j < depCount; j++) {
          const randomDep = candidates[Math.floor(Math.random() * candidates.length)];
          if (!node.temp_deps.includes(randomDep.id)) {
            node.temp_deps.push(randomDep.id);
          }
        }
      }
  
      // Mapping to your specific fields
      node.input1_id = node.temp_deps[0] || null;
      node.input2_id = node.temp_deps[1] || null;
      node.input3_id = node.temp_deps[2] || null;
      delete node.temp_deps;
      node.payload = `Data for ${node.id}`;
    });
  
    // Shuffle the batch to simulate RabbitMQ arrival order
    return data.sort(() => Math.random() - 0.5);
  };
  
  // --- EXECUTION ---
  class ConcurrentDependencyResolver {
    constructor(nodes) {
      this.nodesMap = new Map(nodes.map(node => [node.id, node]));
      this.waitingList = new Map();
      this.inDegree = new Map();
      this.processedCount = 0;
  
      this._initializeGraph();
    }
  
    _initializeGraph() {
      for (const node of this.nodesMap.values()) {
        const deps = [node.input1_id, node.input2_id, node.input3_id].filter(Boolean);
        let pending = 0;
  
        for (const depId of deps) {
          if (this.nodesMap.has(depId)) {
            pending++;
            if (!this.waitingList.has(depId)) this.waitingList.set(depId, []);
            this.waitingList.get(depId).push(node.id);
          }
        }
        this.inDegree.set(node.id, pending);
      }
    }
  
    /**
     * Simulates a calculation or DB fetch
     */
    async processNode(node) {
      // Random delay between 100ms and 500ms to simulate real work
      const delay = Math.floor(Math.random() * 400) + 100;
      return new Promise((resolve) => {
        setTimeout(() => {
          node.is_pending = false; // "Filling" the data
          node.processed = true;
          resolve(node.id);
        }, delay);
      });
    }
  
    async run() {
      let layerCount = 1;
  
      while (this.processedCount < this.nodesMap.size) {
        // 1. Identify current "Generation" 
        // Nodes that aren't done and have 0 pending dependencies
        const layer = Array.from(this.nodesMap.values()).filter(node => {
          return !node.processed && this.inDegree.get(node.id) === 0;
        });
  
        if (layer.length === 0) {
          console.log("\n⚠️  Stop: Remaining nodes have missing dependencies or circular loops.");
          break;
        }
  
        console.log(`\n🚀 [Layer ${layerCount}] Handling ${layer.length} nodes concurrently...`);
        console.log(`   Nodes: ${layer.map(n => n.id).join(', ')}`);
  
        // 2. Process the whole layer at once (Concurrency)
        const results = await Promise.all(layer.map(node => this.processNode(node)));
  
        // 3. Update the graph for the next layer
        for (const finishedId of results) {
          this.processedCount++;
          const dependents = this.waitingList.get(finishedId) || [];
          for (const depId of dependents) {
            const newInDegree = this.inDegree.get(depId) - 1;
            this.inDegree.set(depId, newInDegree);
          }
        }
  
        layerCount++;
      }
  
      console.log(`\n✅ Done! Processed ${this.processedCount} nodes total.`);
    }
  }
  
  // --- Execution ---
  const batch = generateMockData(50); // Using the generator from previous steps
  const runner = new ConcurrentDependencyResolver(batch);
  
  runner.run();