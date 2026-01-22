from fastapi import FastAPI, Request, BackgroundTasks, HTTPException
from fastapi.responses import HTMLResponse, FileResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
import subprocess
import os
import asyncio
import time
from pathlib import Path
from typing import Optional
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

app = FastAPI(title="WAL Benchmark Performance Demo")

BASE_DIR = Path(__file__).parent
STATIC_DIR = BASE_DIR
TEMPLATES_DIR = BASE_DIR

STATIC_DIR.mkdir(exist_ok=True)
TEMPLATES_DIR.mkdir(exist_ok=True)

logger.info(f"Base directory: {BASE_DIR}")
logger.info(f"Static directory: {STATIC_DIR}")
logger.info(f"Templates directory: {TEMPLATES_DIR}")

templates = Jinja2Templates(directory=str(TEMPLATES_DIR))

TEST_COMMAND = "WALRUS_FSYNC=no-fsync WALRUS_DURATION=2m WALRUS_BATCH_SIZE=1000 WALRUS_BACKEND=fd cargo test --release --test multithreaded_benchmark_writes"
VISUALIZE_COMMAND = "python scripts/visualize_throughput_non_interactive.py --file benchmark_throughput.csv"
OUTPUT_IMAGE = "throughput_monitor.png"
CSV_FILE = "benchmark_throughput.csv"

test_status = {
    "running": False, 
    "completed": False, 
    "error": None, 
    "image_url": None,
    "benchmark_started": False,
    "visualization_started": False
}

benchmark_process = None
visualize_process = None

async def run_visualization_monitor():
    global visualize_process
    try:
        logger.info(f"Starting visualization monitor: {VISUALIZE_COMMAND}")
        
        visualize_process = await asyncio.create_subprocess_shell(
            VISUALIZE_COMMAND,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            cwd=str(BASE_DIR)
        )
        
        test_status["visualization_started"] = True
        logger.info("Visualization process started")
        
        image_path = STATIC_DIR / OUTPUT_IMAGE
        last_size = 0
        first_image = True
        no_change_count = 0
        
        while test_status["running"]:
            await asyncio.sleep(1)
            
            if image_path.exists():
                current_size = image_path.stat().st_size
                
                if current_size > 0:
                    if first_image or current_size != last_size:
                        timestamp = int(time.time())
                        test_status["image_url"] = f"/static/{OUTPUT_IMAGE}?t={timestamp}"
                        last_size = current_size
                        first_image = False
                        no_change_count = 0
                        logger.info(f"Image updated: size={current_size} bytes, url={test_status['image_url']}")
                    else:
                        no_change_count += 1
                        if no_change_count % 5 == 0:
                            timestamp = int(time.time())
                            test_status["image_url"] = f"/static/{OUTPUT_IMAGE}?t={timestamp}"
                            logger.info(f"Image refreshed (no change): size={current_size} bytes, url={test_status['image_url']}")
            else:
                logger.debug(f"Image file not found yet: {image_path}")
        
        logger.info("Visualization monitoring stopped")
        
    except Exception as e:
        logger.error(f"Error during visualization monitoring: {str(e)}")
        test_status["error"] = f"Visualization error: {str(e)}"

async def run_benchmark():
    global benchmark_process
    try:
        test_status["running"] = True
        test_status["completed"] = False
        test_status["error"] = None
        test_status["image_url"] = None
        test_status["benchmark_started"] = False
        test_status["visualization_started"] = False
        
        logger.info(f"Running benchmark command: {TEST_COMMAND}")
        
        benchmark_process = await asyncio.create_subprocess_shell(
            TEST_COMMAND,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            cwd=str(BASE_DIR)
        )
        
        test_status["benchmark_started"] = True
        logger.info("Benchmark process started, waiting 1 second before starting visualization")
        
        await asyncio.sleep(1)
        
        if test_status["running"]:
            asyncio.create_task(run_visualization_monitor())
        
        stdout, stderr = await benchmark_process.communicate()
        
        if benchmark_process.returncode != 0:
            error_msg = stderr.decode() if stderr else "Unknown error"
            logger.error(f"Benchmark failed: {error_msg}")
            test_status["error"] = error_msg
            test_status["running"] = False
            return
        
        logger.info("Benchmark completed successfully")
        
        csv_path = BASE_DIR / CSV_FILE
        if not csv_path.exists():
            logger.warning(f"CSV file not found: {csv_path}")
            test_status["error"] = f"CSV file not generated: {CSV_FILE}"
            test_status["running"] = False
            return
        
        image_path = STATIC_DIR / OUTPUT_IMAGE
        if image_path.exists():
            timestamp = int(time.time())
            test_status["image_url"] = f"/static/{OUTPUT_IMAGE}?t={timestamp}"
            logger.info(f"Final image: {test_status['image_url']}")
            logger.info(f"Image path: {image_path}")
            logger.info(f"Image size: {image_path.stat().st_size} bytes")
        
        test_status["completed"] = True
        test_status["running"] = False
        
    except Exception as e:
        logger.error(f"Error during benchmark: {str(e)}")
        test_status["error"] = str(e)
        test_status["running"] = False

@app.get("/", response_class=HTMLResponse)
async def index(request: Request):
    return templates.TemplateResponse("index.html", {
        "request": request,
        "test_command": TEST_COMMAND,
        "visualize_command": VISUALIZE_COMMAND
    })

@app.post("/api/run-benchmark")
async def run_benchmark_endpoint(background_tasks: BackgroundTasks):
    if test_status["running"]:
        raise HTTPException(status_code=400, detail="Benchmark is already running")
    
    background_tasks.add_task(run_benchmark)
    return {"message": "Benchmark started", "status": "running"}

@app.get("/api/status")
async def get_status():
    return {
        "running": test_status["running"],
        "completed": test_status["completed"],
        "error": test_status["error"],
        "image_url": test_status["image_url"],
        "benchmark_started": test_status["benchmark_started"],
        "visualization_started": test_status["visualization_started"]
    }

async def stop_and_cleanup():
    global benchmark_process, visualize_process
    
    logger.info("Stopping all processes and cleaning up files...")
    
    if benchmark_process and benchmark_process.returncode is None:
        logger.info("Terminating benchmark process...")
        try:
            benchmark_process.terminate()
            try:
                await asyncio.wait_for(benchmark_process.wait(), timeout=5.0)
            except asyncio.TimeoutError:
                logger.warning("Benchmark process did not terminate, killing...")
                benchmark_process.kill()
                await benchmark_process.wait()
            logger.info("Benchmark process terminated")
        except Exception as e:
            logger.error(f"Error terminating benchmark process: {e}")
    
    if visualize_process and visualize_process.returncode is None:
        logger.info("Terminating visualization process...")
        try:
            visualize_process.terminate()
            try:
                await asyncio.wait_for(visualize_process.wait(), timeout=5.0)
            except asyncio.TimeoutError:
                logger.warning("Visualization process did not terminate, killing...")
                visualize_process.kill()
                await visualize_process.wait()
            logger.info("Visualization process terminated")
        except Exception as e:
            logger.error(f"Error terminating visualization process: {e}")
    
    csv_path = BASE_DIR / CSV_FILE
    if csv_path.exists():
        try:
            csv_path.unlink()
            logger.info(f"Deleted CSV file: {csv_path}")
        except Exception as e:
            logger.error(f"Error deleting CSV file: {e}")
    
    image_path = STATIC_DIR / OUTPUT_IMAGE
    if image_path.exists():
        try:
            image_path.unlink()
            logger.info(f"Deleted image file: {image_path}")
        except Exception as e:
            logger.error(f"Error deleting image file: {e}")
    
    test_status["running"] = False
    test_status["completed"] = False
    test_status["error"] = None
    test_status["image_url"] = None
    test_status["benchmark_started"] = False
    test_status["visualization_started"] = False
    
    logger.info("Cleanup completed")

@app.post("/api/reset")
async def reset_status():
    try:
        await stop_and_cleanup()
        return {"message": "Status reset and cleanup completed"}
    except Exception as e:
        logger.error(f"Error during reset: {str(e)}")
        return {"message": f"Reset failed: {str(e)}"}

@app.get("/api/check-image")
async def check_image():
    image_path = STATIC_DIR / OUTPUT_IMAGE
    if image_path.exists():
        return {"exists": True, "size": image_path.stat().st_size, "path": str(image_path)}
    else:
        return {"exists": False, "path": str(image_path)}

app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)