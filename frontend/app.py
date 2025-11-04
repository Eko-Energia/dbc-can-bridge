import streamlit as st
import pandas as pd
from pymongo import MongoClient
from datetime import datetime, timezone
import time
import plotly.express as px
import os

# Connect to MongoDB using environment variable
mongo_host = os.getenv('MONGO_HOST', 'localhost')
client = MongoClient(mongo_host, 27017, username='admin', password='password')
db = client['perla_monitor']
collection = db['jsons']

# Tabs for views at the top
tab1, tab2 = st.tabs(["Current Parameters", "Historical Data"])

with tab1:
    st.title("Perla Monitor - Current Parameters")

    # Slider for refresh interval (in seconds)
    refresh_interval = st.slider("Refresh Interval (seconds)", min_value=1, max_value=300, value=10)

    # Toggle for auto-refresh
    auto_refresh = st.checkbox("Enable Auto-Refresh", value=True)

    # Manual refresh button
    if st.button("Manual Refresh"):
        st.rerun()

    # Function to fetch latest data
    def get_latest_data():
        # Aggregate to get the latest document per frame
        pipeline = [
            {"$sort": {"receivedAt": -1}},
            {"$group": {"_id": "$frame", "latest": {"$first": "$$ROOT"}}},
            {"$replaceRoot": {"newRoot": "$latest"}}
        ]
        results = list(collection.aggregate(pipeline))
        return results

    # Display data
    placeholder = st.empty()

    def update_table():
        data = get_latest_data()
        if data:
            df = pd.DataFrame([{
                "Frame": doc.get("frame", ""),
                "Val": doc.get("val", ""),
                "Unit": doc.get("unit", ""),
                "Time": datetime.fromtimestamp(doc.get("time", 0), tz=timezone.utc).strftime('%Y-%m-%d %H:%M:%S'),
                "Received At": doc.get("receivedAt", "").strftime('%Y-%m-%d %H:%M:%S') if doc.get("receivedAt") else ""
            } for doc in data])
            placeholder.dataframe(df)
        else:
            placeholder.write("No data available")

    # Initial load
    update_table()

    # Note: Auto-refresh with infinite loop may not work well in Streamlit; consider using st_autorefresh library for better handling
    if auto_refresh:
        while True:
            time.sleep(refresh_interval)
            update_table()
            st.rerun()

with tab2:
    st.title("Historical Data Chart")

    # Get unique frames (parameters)
    parameters = collection.distinct("frame")
    selected_parameter = st.selectbox("Select Parameter", parameters)

    if selected_parameter:
        # Fetch historical data for selected parameter
        data = list(collection.find({"frame": selected_parameter}).sort("time", 1))
        if data:
            df = pd.DataFrame([{
                "Time": datetime.fromtimestamp(doc.get("time", 0), tz=timezone.utc),
                "Val": doc.get("val", 0),
                "Unit": doc.get("unit", "")
            } for doc in data])
            # Plot line chart for the selected parameter
            fig = px.line(df, x="Time", y="Val", title=f"Historical Data for {selected_parameter} ({df['Unit'].iloc[0]})")
            st.plotly_chart(fig)
        else:
            st.write("No historical data available for this parameter")
    else:
        st.write("Select a parameter to view historical data")